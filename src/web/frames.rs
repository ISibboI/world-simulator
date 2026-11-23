//! The frame history of the web version in IndexedDB (see `viewer::history`): one database per
//! run, `world-simulator-frames-<random>`, with one object store of the encoded frames by their
//! number. It is deleted when the run ends (restart or the page closes). The browser may evict
//! it under storage pressure; frames that are gone can no longer be shown.
//!
//! IndexedDB on the page rather than the Origin Private File System in the worker: the view
//! needs past frames while the worker is busy with a step (seconds at 6 subdivisions), and the
//! worker could only answer between steps; and on the page OPFS has no cheap append (a writable
//! stream commits by copying the file).
//!
//! A page that is closed cannot be relied on to delete its database, so each page holds a Web
//! Lock named after its run's database while the run lives, and [`delete_stale`] deletes, when a
//! page starts, the databases whose lock nobody holds: those of closed pages, not those of other
//! open tabs.
//!
//! If IndexedDB cannot be opened (some private modes), the frames are kept in memory, up to
//! [`MEMORY_CAPACITY`].

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use tracing::{error, info, warn};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;

use crate::viewer::{FrameStore, MemoryStore};

/// Prefix of the names of the databases.
const PREFIX: &str = "world-simulator-frames-";
/// The object store of the frames.
const FRAMES: &str = "frames";
/// Bytes the history may keep in IndexedDB. Browsers grant a share of the free disk (usually
/// gigabytes); a failed write (quota) stops the history earlier.
const CAPACITY: u64 = 1 << 30;
/// Bytes the history may keep in memory if IndexedDB is not available.
pub const MEMORY_CAPACITY: u64 = 256 << 20;

/// The state shared with the callbacks of IndexedDB.
struct Shared {
    database: Option<web_sys::IdbDatabase>,
    /// Frames to write once the database is open.
    queued: Vec<(usize, Vec<u8>)>,
    /// A read to start once the database is open.
    queued_read: Option<Range<usize>>,
    /// Frames in memory, if IndexedDB is not available.
    fallback: Option<MemoryStore>,
    result: Option<Result<Vec<Vec<u8>>, String>>,
    /// The number of frames the running read asks for.
    expected: usize,
    failed: Option<usize>,
    wake: Box<dyn Fn()>,
    /// The handlers of writes and reads, one of each per run for all requests (so that
    /// handlers which never fire do not pile up). They find the request in their event and
    /// hold the state only weakly, so a dropped store is freed.
    on_write_abort: JsValue,
    on_read_success: JsValue,
    on_read_error: JsValue,
}

/// The frames of one run in IndexedDB.
pub struct IdbStore {
    name: String,
    shared: Rc<RefCell<Shared>>,
    /// Releases the Web Lock of the database.
    release: Option<js_sys::Function>,
}

fn message(value: &JsValue) -> String {
    value
        .dyn_ref::<js_sys::Error>()
        .map(|error| String::from(error.message()))
        .or_else(|| {
            value
                .dyn_ref::<web_sys::DomException>()
                .map(|exception| format!("{}: {}", exception.name(), exception.message()))
        })
        .unwrap_or_else(|| format!("{value:?}"))
}

fn factory() -> Result<web_sys::IdbFactory, String> {
    let global = js_sys::global();
    let factory =
        js_sys::Reflect::get(&global, &"indexedDB".into()).map_err(|error| message(&error))?;
    factory
        .dyn_into::<web_sys::IdbFactory>()
        .map_err(|_| "IndexedDB is not available".to_owned())
}

/// The Web Lock manager of the page, if the browser has one.
fn locks() -> Option<JsValue> {
    let navigator = js_sys::Reflect::get(&js_sys::global(), &"navigator".into()).ok()?;
    js_sys::Reflect::get(&navigator, &"locks".into())
        .ok()
        .filter(|locks| !locks.is_undefined())
}

fn call(object: &JsValue, method: &str, args: &[&JsValue]) -> Result<JsValue, String> {
    let function = js_sys::Reflect::get(object, &method.into())
        .map_err(|error| message(&error))?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| format!("no {method}"))?;
    let array = js_sys::Array::new();
    for arg in args {
        array.push(arg);
    }
    function
        .apply(object, &array)
        .map_err(|error| message(&error))
}

/// Holds the Web Lock `name` until the returned function is called (or the page closes).
fn hold_lock(name: &str) -> Option<js_sys::Function> {
    let locks = locks()?;
    let mut release = None;
    let held = js_sys::Promise::new(&mut |resolve, _reject| release = Some(resolve));
    let callback = Closure::once_into_js(move |_lock: JsValue| held);
    if let Err(error) = call(&locks, "request", &[&name.into(), &callback]) {
        warn!("Could not hold the lock of the frame history: {error}");
        return None;
    }
    release
}

/// Deletes the frame databases of closed pages: those whose Web Lock nobody holds. Without
/// Web Locks or the list of databases (older browsers), nothing is deleted.
pub async fn delete_stale() {
    if let Err(error) = try_delete_stale().await {
        info!("Old frame histories were not cleaned up: {error}");
    }
}

async fn try_delete_stale() -> Result<(), String> {
    let factory = factory()?;
    let locks = locks().ok_or("no Web Locks")?;
    let databases = call(&factory, "databases", &[])?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| "databases() is not a promise".to_owned())?;
    let databases = JsFuture::from(databases)
        .await
        .map_err(|error| message(&error))?;
    let query = call(&locks, "query", &[])?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| "query() is not a promise".to_owned())?;
    let snapshot = JsFuture::from(query)
        .await
        .map_err(|error| message(&error))?;
    let names = |list: JsValue| -> Vec<String> {
        js_sys::Array::from(&list)
            .iter()
            .filter_map(|item| {
                js_sys::Reflect::get(&item, &"name".into())
                    .ok()?
                    .as_string()
            })
            .collect()
    };
    let mut held = Vec::new();
    for key in ["held", "pending"] {
        held.extend(names(
            js_sys::Reflect::get(&snapshot, &key.into()).unwrap_or(JsValue::UNDEFINED),
        ));
    }
    for name in names(databases) {
        if name.starts_with(PREFIX) && !held.contains(&name) {
            info!(name, "Deleting the frame history of a closed page");
            if let Err(error) = factory.delete_database(&name) {
                warn!("Could not delete {name}: {}", message(&error));
            }
        }
    }
    Ok(())
}

impl IdbStore {
    /// Opens a new database for a run; `wake` wakes the view when a read completes.
    pub fn open(wake: Box<dyn Fn()>) -> Self {
        let name = format!(
            "{PREFIX}{:x}{:x}",
            js_sys::Date::now() as u64,
            (js_sys::Math::random() * 4_294_967_296.0) as u64
        );
        let shared = Rc::new(RefCell::new(Shared {
            database: None,
            queued: Vec::new(),
            queued_read: None,
            fallback: None,
            result: None,
            expected: 0,
            failed: None,
            wake,
            on_write_abort: JsValue::UNDEFINED,
            on_read_success: JsValue::UNDEFINED,
            on_read_error: JsValue::UNDEFINED,
        }));
        install_handlers(&shared);
        let release = hold_lock(&name);
        if let Err(error) = open(&name, &shared) {
            fall_back(&shared, &error);
        }
        Self {
            name,
            shared,
            release,
        }
    }
}

/// Keeps the frames in memory from now on.
fn fall_back(shared: &Rc<RefCell<Shared>>, error: &str) {
    warn!("IndexedDB is not available for the frame history, keeping it in memory: {error}");
    let mut state = shared.borrow_mut();
    let mut memory = MemoryStore::new(MEMORY_CAPACITY);
    for (index, frame) in std::mem::take(&mut state.queued) {
        memory.append(index, frame);
    }
    if let Some(range) = state.queued_read.take() {
        memory.read(range);
        state.result = memory.take_read();
        (state.wake)();
    }
    state.fallback = Some(memory);
}

/// The target of an IndexedDB event: its request or transaction.
fn target<T: JsCast>(event: &web_sys::Event) -> Option<T> {
    event.target()?.dyn_into::<T>().ok()
}

/// The error of a failed request, as text.
fn request_error(request: Option<web_sys::IdbRequest>) -> String {
    request
        .and_then(|request| request.error().ok().flatten())
        .map_or_else(|| "unknown error".to_owned(), |error| message(&error))
}

/// Property of a write transaction with the number of its frame.
const FRAME_PROPERTY: &str = "worldSimulatorFrame";

fn install_handlers(shared: &Rc<RefCell<Shared>>) {
    let weak = Rc::downgrade(shared);
    let on_write_abort = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let Some(shared) = weak.upgrade() else {
            return;
        };
        let transaction = target::<web_sys::IdbTransaction>(&event);
        let index = transaction
            .as_ref()
            .and_then(|transaction| {
                js_sys::Reflect::get(transaction, &FRAME_PROPERTY.into())
                    .ok()?
                    .as_f64()
            })
            .map_or(0, |index| index as usize);
        let error = transaction
            .and_then(|transaction| transaction.error())
            .map_or_else(|| "aborted".to_owned(), |error| message(&error));
        fail_write(&shared, index, &error);
    })
    .into_js_value();
    let weak = Rc::downgrade(shared);
    let on_read_success =
        Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            let Some(shared) = weak.upgrade() else {
                return;
            };
            let frames: Vec<Vec<u8>> = target::<web_sys::IdbRequest>(&event)
                .and_then(|request| request.result().ok())
                .map(|result| {
                    js_sys::Array::from(&result)
                        .iter()
                        .map(|frame| js_sys::Uint8Array::new(&frame).to_vec())
                        .collect()
                })
                .unwrap_or_default();
            let expected = shared.borrow().expected;
            finish_read(
                &shared,
                if frames.len() == expected {
                    Ok(frames)
                } else {
                    Err("the browser deleted the stored frames".to_owned())
                },
            );
        })
        .into_js_value();
    let weak = Rc::downgrade(shared);
    let on_read_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        if let Some(shared) = weak.upgrade() {
            let error = request_error(target(&event));
            finish_read(&shared, Err(format!("reading the stored frames: {error}")));
        }
    })
    .into_js_value();
    let mut state = shared.borrow_mut();
    state.on_write_abort = on_write_abort;
    state.on_read_success = on_read_success;
    state.on_read_error = on_read_error;
}

fn fail_write(shared: &Rc<RefCell<Shared>>, index: usize, error: &str) {
    let mut state = shared.borrow_mut();
    if state.failed.is_none_or(|failed| index < failed) {
        warn!(
            frame = index,
            "Could not store a frame of the history: {error}"
        );
        state.failed = Some(index);
    }
}

fn finish_read(shared: &Rc<RefCell<Shared>>, result: Result<Vec<Vec<u8>>, String>) {
    let mut state = shared.borrow_mut();
    state.result = Some(result);
    (state.wake)();
}

fn open(name: &str, shared: &Rc<RefCell<Shared>>) -> Result<(), String> {
    let request = factory()?
        .open_with_u32(name, 1)
        .map_err(|error| message(&error))?;
    // These fire once per run; they hold the state only weakly.
    let upgrade = Closure::once_into_js(move |event: web_sys::Event| {
        if let Some(request) = target::<web_sys::IdbOpenDbRequest>(&event)
            && let Ok(result) = request.result()
            && let Ok(database) = result.dyn_into::<web_sys::IdbDatabase>()
            && let Err(error) = database.create_object_store(FRAMES)
        {
            error!("Could not create the frame store: {}", message(&error));
        }
    });
    request.set_onupgradeneeded(Some(upgrade.unchecked_ref()));
    let weak = Rc::downgrade(shared);
    let success = Closure::once_into_js(move |event: web_sys::Event| {
        let database = target::<web_sys::IdbOpenDbRequest>(&event)
            .and_then(|request| request.result().ok())
            .and_then(|result| result.dyn_into::<web_sys::IdbDatabase>().ok());
        let Some(shared) = weak.upgrade() else {
            // The run ended while the database opened: close it, so that its deletion
            // (requested when the run ended) can go ahead.
            if let Some(database) = database {
                database.close();
            }
            return;
        };
        let Some(database) = database else {
            fall_back(&shared, "the database did not open");
            return;
        };
        let (queued, read) = {
            let mut state = shared.borrow_mut();
            state.database = Some(database);
            (std::mem::take(&mut state.queued), state.queued_read.take())
        };
        for (index, frame) in queued {
            write(&shared, index, &frame);
        }
        if let Some(range) = read {
            start_read(&shared, range);
        }
    });
    request.set_onsuccess(Some(success.unchecked_ref()));
    let weak = Rc::downgrade(shared);
    let failure = Closure::once_into_js(move |event: web_sys::Event| {
        if let Some(shared) = weak.upgrade() {
            fall_back(&shared, &request_error(target(&event)));
        }
    });
    request.set_onerror(Some(failure.unchecked_ref()));
    Ok(())
}

fn object_store(
    shared: &Rc<RefCell<Shared>>,
    mode: web_sys::IdbTransactionMode,
) -> Result<(web_sys::IdbTransaction, web_sys::IdbObjectStore), String> {
    let state = shared.borrow();
    let database = state.database.as_ref().ok_or("the database is not open")?;
    let transaction = database
        .transaction_with_str_and_mode(FRAMES, mode)
        .map_err(|error| message(&error))?;
    let store = transaction
        .object_store(FRAMES)
        .map_err(|error| message(&error))?;
    Ok((transaction, store))
}

/// Writes a frame; a failure (e.g. the quota) marks it as failed.
fn write(shared: &Rc<RefCell<Shared>>, index: usize, frame: &[u8]) {
    let result = object_store(shared, web_sys::IdbTransactionMode::Readwrite).and_then(
        |(transaction, store)| {
            let _ = js_sys::Reflect::set(
                &transaction,
                &FRAME_PROPERTY.into(),
                &JsValue::from(index as u32),
            );
            transaction.set_onabort(Some(shared.borrow().on_write_abort.unchecked_ref()));
            let value = js_sys::Uint8Array::from(frame);
            store
                .put_with_key(&value, &JsValue::from(index as u32))
                .map(|_| ())
                .map_err(|error| message(&error))
        },
    );
    if let Err(error) = result {
        fail_write(shared, index, &error);
    }
}

fn start_read(shared: &Rc<RefCell<Shared>>, range: Range<usize>) {
    shared.borrow_mut().expected = range.len();
    let request =
        object_store(shared, web_sys::IdbTransactionMode::Readonly).and_then(|(_, store)| {
            let keys = web_sys::IdbKeyRange::bound(
                &JsValue::from(range.start as u32),
                &JsValue::from((range.end - 1) as u32),
            )
            .map_err(|error| message(&error))?;
            store
                .get_all_with_key(&keys)
                .map_err(|error| message(&error))
        });
    match request {
        Ok(request) => {
            let state = shared.borrow();
            request.set_onsuccess(Some(state.on_read_success.unchecked_ref()));
            request.set_onerror(Some(state.on_read_error.unchecked_ref()));
        }
        Err(error) => finish_read(shared, Err(error)),
    }
}

impl FrameStore for IdbStore {
    fn append(&mut self, index: usize, frame: Vec<u8>) {
        let mut state = self.shared.borrow_mut();
        if let Some(memory) = &mut state.fallback {
            memory.append(index, frame);
        } else if state.database.is_none() {
            state.queued.push((index, frame));
        } else {
            drop(state);
            write(&self.shared, index, &frame);
        }
    }

    fn read(&mut self, range: Range<usize>) {
        let mut state = self.shared.borrow_mut();
        if let Some(memory) = &mut state.fallback {
            memory.read(range);
            state.result = memory.take_read();
        } else if state.database.is_none() {
            state.queued_read = Some(range);
        } else {
            drop(state);
            start_read(&self.shared, range);
        }
    }

    fn take_read(&mut self) -> Option<Result<Vec<Vec<u8>>, String>> {
        self.shared.borrow_mut().result.take()
    }

    fn failed_write(&mut self) -> Option<usize> {
        self.shared.borrow().failed
    }

    fn capacity(&self) -> u64 {
        if self.shared.borrow().fallback.is_some() {
            MEMORY_CAPACITY
        } else {
            CAPACITY
        }
    }

    fn describe(&self) -> &'static str {
        if self.shared.borrow().fallback.is_some() {
            "in memory"
        } else {
            "in the browser's storage (IndexedDB)"
        }
    }
}

impl Drop for IdbStore {
    fn drop(&mut self) {
        // Later callbacks find no database and no one to wake.
        let mut state = self.shared.borrow_mut();
        state.wake = Box::new(|| {});
        if let Some(database) = state.database.take() {
            database.close();
        }
        state.fallback = None;
        drop(state);
        match factory().and_then(|factory| {
            factory
                .delete_database(&self.name)
                .map_err(|error| message(&error))
        }) {
            Ok(_) => info!(name = self.name, "Frame history deleted"),
            Err(error) => warn!("Could not delete the frame history {}: {error}", self.name),
        }
        if let Some(release) = self.release.take() {
            let _ = release.call0(&JsValue::NULL);
        }
    }
}
