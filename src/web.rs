//! The web version (`wasm32`): the page shows the same 3D view as the desktop window, and the
//! simulation runs in a Web Worker so that the view stays responsive while a step runs.
//!
//! Both are the same wasm module: `web/index.html` loads it on the page, and `web/worker.js`
//! loads it again in a module worker. [`start`] tells them apart by their global scope. The
//! parameters are the command-line options, given as URL query parameters
//! (`?seed=3&subdivisions=5&no-erosion&set=basal_drag=8e14`, see [`crate::query`]); the worker
//! gets the same query string, after the files of the page's build that it loads (`module` and
//! `wasm`, see [`crate::query::worker_query`]). Their names carry content hashes (Trunk's
//! `filehash`), so that a page never runs with cached files of another build (issue #122).
//! Failures while the page loads are shown in its loading banner by the script in
//! `web/index.html`. It sends `{ generating: true }` before it builds the planet, then a
//! surface update after every step as
//! `{ time, cells, plates }` (the attributes of the cells as [`viewer::CellInfo`]s, the
//! [`viewer::PlateInfo`] of every plate, `frame`, the state encoded for the page's frame history
//! by the worker's [`viewer::FrameEncoder`] (not for a state sent again), and with the first
//! update also `directions` and `faces`, the mesh),
//! with the buffers transferred rather than copied, or `{ error }` if the simulation fails. A restart from the
//! parameter panel terminates the worker and starts a new one with the new query string, which
//! also replaces the query of the page URL, so that the setting can be shared as a link. Each
//! update after the first also carries `step_seconds`, the time of the step in the worker.
//!
//! Updates also carry the extra fields of the render mode ([`viewer::Layer`]s, each as a
//! transferred `Float32Array` buffer under its key): at first those of the mode in the
//! worker's query, then those the page requests with `{ layers }` (the bits of
//! [`viewer::Layers`]). The worker runs one step at a time (scheduled with `setTimeout`) so that
//! such messages reach it between steps; it then sends its current state again with the new
//! fields at once, also after the run is complete.
//!
//! The render mode is remembered like the subdivisions: the URL's `render-mode` wins over the
//! one in the local storage (stored when the user picks a mode), which wins over the default.
//! Picking a mode puts it into the page URL at once, without a restart.
//!
//! The time bar pauses the worker with `{ paused: true/false }` and asks for one step while paused
//! with `{ step: true }`; both take effect between steps. The page keeps the frame history of the
//! run in IndexedDB ([`frames`]).
//!
//! The page records its state as `data-` attributes of its root element (see [`mark`]), for the
//! browser tests in `web/tests/`.
//!
//! Without `subdivisions` in the URL, the page starts with the subdivisions remembered in the
//! local storage from the last restart, or else with a guess for the device (see
//! [`crate::device`]). The worker always gets them explicitly; the page URL only when the user
//! chose them.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use anyhow::{Context, anyhow, ensure};
use clap::Parser;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use tracing::{error, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::device::{self, DeviceSignals, Source};
use crate::query::{args_to_query, query_to_args, without_worker_files, worker_query};
use crate::viewer::{
    self, CellInfo, Fields, FrameEncoder, FrameStore, Launch, Layer, Layers, MeshUpdate, PlateInfo,
    RenderModeId, SimulationControl, SimulationEvent, SurfaceUpdate, UiOptions, has_option,
    with_default,
};

mod frames;
use crate::{Cli, Run, build_planet};

/// Id of the canvas in `web/index.html`.
const CANVAS_ID: &str = "world_simulator";
/// Id of the element in `web/index.html` that shows the loading state and errors until the
/// view exists (the view then shows its own banner, see `viewer::banner`).
const STATUS_ID: &str = "status";
/// Script of the simulation worker, relative to the page.
const WORKER_SCRIPT: &str = "./worker.js";
/// What the banner says after a failure to load a file, also in `web/index.html` and
/// `web/worker.js`.
const RELOAD_HINT: &str = "Reload the page.";

/// Entry point of the page and of the worker.
pub fn start() {
    console_error_panic_hook::set_once();
    init_logging();
    let global = js_sys::global();
    if let Some(scope) = global.dyn_ref::<web_sys::DedicatedWorkerGlobalScope>() {
        if let Err(error) = run_worker(scope) {
            report_failure(scope, &error);
        }
    } else {
        wasm_bindgen_futures::spawn_local(async {
            if let Err(error) = run_page().await {
                error!("{error:#}");
                show_status(&format!("{error:#}"));
            }
        });
    }
}

/// Logs to the browser console at `info` level. The default timer of `tracing_subscriber` reads
/// the system time, which panics on wasm.
fn init_logging() {
    let layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .without_time()
        .with_writer(tracing_web::MakeWebConsoleWriter::new());
    tracing_subscriber::registry()
        .with(layer)
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .init();
}

/// Parses the query string of the page (or worker) URL into the command-line options, and
/// returns them with the arguments they came from.
fn parse_query(search: &str) -> anyhow::Result<Launch> {
    let args = query_to_args(search)?;
    let cli =
        Cli::try_parse_from(&args).map_err(|error| anyhow!("invalid URL parameters: {error}"))?;
    Ok(Launch { cli, args })
}

fn set(object: &js_sys::Object, key: &str, value: &JsValue) {
    // Setting a property on a plain object cannot fail.
    let _ = js_sys::Reflect::set(object, &key.into(), value);
}

fn get(object: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(object, &key.into()).unwrap_or(JsValue::UNDEFINED)
}

fn document() -> anyhow::Result<web_sys::Document> {
    web_sys::window()
        .and_then(|window| window.document())
        .context("no document")
}

/// Shows `text` in the status element, or hides it if `text` is empty.
fn show_status(text: &str) {
    if let Ok(Some(element)) = document().map(|document| document.get_element_by_id(STATUS_ID)) {
        element.set_text_content(Some(text));
        let display = if text.is_empty() { "none" } else { "block" };
        if let Some(element) = element.dyn_ref::<web_sys::HtmlElement>() {
            let _ = element.style().set_property("display", display);
        }
    }
}

/// Records a state of the page as the attribute `data-<name>` of its root element: for the
/// browser tests (`web/tests/`), which read `data-first-frame` (the view drew its first frame),
/// `data-surface-updates` (the number of states from the simulation worker),
/// `data-simulation-failed` (why the simulation failed) and `data-frame-history` (where the frame
/// history is kept, see [`frames`]). Harmless otherwise.
fn mark(name: &str, value: &str) {
    if let Ok(Some(root)) = document().map(|document| document.document_element())
        && let Err(error) = root.set_attribute(&format!("data-{name}"), value)
    {
        error!("Could not set data-{name}: {error:?}");
    }
}

/// Adapts the user interface to the browser: body text at the root font size of the page (the
/// user's font size setting, usually 16 px), and larger targets if the main pointer is coarse
/// (a touch screen).
fn ui_options(window: &web_sys::Window, default_subdivisions: u32) -> UiOptions {
    let body_text_size = window
        .document()
        .and_then(|document| document.document_element())
        .and_then(|root| window.get_computed_style(&root).ok().flatten())
        .and_then(|style| style.get_property_value("font-size").ok())
        .and_then(|size| size.trim_end_matches("px").trim().parse::<f32>().ok())
        .filter(|size| size.is_finite() && *size > 0.0);
    let touch = media_matches(window, "(pointer: coarse)");
    let options = UiOptions {
        body_text_size,
        touch,
        default_subdivisions: Some(default_subdivisions),
        // 7 would need ×4 the memory and upload of 6.
        max_recommended_subdivisions: Some(6),
    };
    info!(?options, "User interface options");
    options
}

fn media_matches(window: &web_sys::Window, query: &str) -> bool {
    window
        .match_media(query)
        .ok()
        .flatten()
        .is_some_and(|query| query.matches())
}

/// What the browser tells about the device, for the guess of the subdivisions. Properties
/// that a browser does not have are left out.
fn device_signals(window: &web_sys::Window) -> DeviceSignals {
    let navigator: JsValue = window.navigator().into();
    let signals = DeviceSignals {
        user_agent: get(&navigator, "userAgent").as_string().unwrap_or_default(),
        mobile: get(&get(&navigator, "userAgentData"), "mobile").as_bool(),
        max_touch_points: get(&navigator, "maxTouchPoints")
            .as_f64()
            .map_or(0, |points| points as u32),
        touch_only: media_matches(window, "(pointer: coarse)")
            && media_matches(window, "(hover: none)"),
        device_memory: get(&navigator, "deviceMemory").as_f64(),
    };
    info!(?signals, guess = signals.guess(), "Device");
    signals
}

/// The local storage of the page, if the browser allows it (not in some private modes, or if
/// the user blocks site data).
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// The remembered subdivisions, if there are valid ones.
fn stored_subdivisions() -> Option<u32> {
    let text = local_storage()?
        .get_item(device::STORAGE_KEY)
        .ok()
        .flatten();
    device::parse_stored(text.as_deref())
}

/// Remembers the subdivisions the user started a simulation with, or forgets them if they are
/// the guess. Failures (no storage, quota) only cost the memory.
fn store_subdivisions(subdivisions: u32, guess: u32) {
    let Some(storage) = local_storage() else {
        return;
    };
    let result = match device::to_store(subdivisions, guess) {
        Some(subdivisions) => storage.set_item(device::STORAGE_KEY, &subdivisions.to_string()),
        None => storage.remove_item(device::STORAGE_KEY),
    };
    if let Err(error) = result {
        error!("Could not remember the subdivisions: {error:?}");
    }
}

/// The remembered render mode, if any (the view checks the name).
fn stored_render_mode() -> Option<String> {
    local_storage()?
        .get_item(viewer::RENDER_MODE_STORAGE_KEY)
        .ok()
        .flatten()
}

/// Remembers the render mode the user picked, or forgets it if it is the default, and puts it
/// into the page URL in place of the one there (the rest of the URL stays as it is).
fn remember_render_mode(mode: RenderModeId) {
    let search = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .unwrap_or_default();
    match query_to_args(&search) {
        Ok(args) => replace_query(&args_to_query(&viewer::with_render_mode(&args, mode))),
        Err(error) => error!("Could not update the URL: {error:#}"),
    }
    let Some(storage) = local_storage() else {
        return;
    };
    let result = match viewer::to_store(mode) {
        Some(name) => storage.set_item(viewer::RENDER_MODE_STORAGE_KEY, name),
        None => storage.remove_item(viewer::RENDER_MODE_STORAGE_KEY),
    };
    if let Err(error) = result {
        error!("Could not remember the render mode: {error:?}");
    }
}

/// The view's settings (see `viewer::Settings`) in the local storage.
struct LocalStorageSettings;

impl viewer::Settings for LocalStorageSettings {
    fn get(&self, key: &str) -> Option<String> {
        local_storage()?.get_item(key).ok().flatten()
    }

    fn set(&mut self, key: &str, value: &str) {
        if let Some(storage) = local_storage()
            && let Err(error) = storage.set_item(key, value)
        {
            error!("Could not remember {key}: {error:?}");
        }
    }
}

/// Sends requests of the page to the running simulation worker.
struct WorkerControl(web_sys::Worker);

impl WorkerControl {
    fn send(&self, key: &str, value: &JsValue) {
        let message = js_sys::Object::new();
        set(&message, key, value);
        // A terminated worker ignores messages.
        if let Err(error) = self.0.post_message(&message) {
            error!("Could not send the request to the simulation worker: {error:?}");
        }
    }
}

impl SimulationControl for WorkerControl {
    fn request_layers(&mut self, layers: Layers) {
        self.send("layers", &layers.0.into());
    }

    fn set_paused(&mut self, paused: bool) {
        self.send("paused", &paused.into());
    }

    fn step(&mut self) {
        self.send("step", &true.into());
    }
}

/// A control for a worker that could not start.
struct NoControl;

impl SimulationControl for NoControl {
    fn request_layers(&mut self, _layers: Layers) {}

    fn set_paused(&mut self, _paused: bool) {}

    fn step(&mut self) {}
}

/// Starts the 3D view on the page and the simulation worker.
async fn run_page() -> anyhow::Result<()> {
    let window = web_sys::window().context("no window")?;
    let search = window
        .location()
        .search()
        .map_err(|error| anyhow!("reading the URL: {error:?}"))?;
    let url_args = query_to_args(&search)?;
    // Opening the page without a seed makes a new planet; its seed goes into the URL, so that
    // reloading or sharing the page makes the same planet again.
    let url_args = if has_option(&url_args, "seed") {
        url_args
    } else {
        let args = viewer::with_random_seed(&url_args);
        replace_query(&args_to_query(&args));
        args
    };
    let guess = device_signals(&window).guess();
    let source = device::source(has_option(&url_args, "subdivisions"), stored_subdivisions());
    info!(?source, "Subdivisions");
    let parse = |args: &[String]| {
        let complete = with_default(args, "subdivisions", &guess.to_string());
        Cli::try_parse_from(&complete).map_err(|error| anyhow!("invalid URL parameters: {error}"))
    };
    let mut launch = Launch {
        cli: parse(&url_args)?,
        args: url_args.clone(),
    };
    if let Source::Stored(subdivisions) = source {
        let mut args = url_args;
        args.insert(1, format!("--subdivisions={subdivisions}"));
        // A remembered value that does not fit the options of the link (e.g. too few cells
        // for its plates) is forgotten rather than failing on every visit.
        match parse(&args).and_then(|cli| cli.validate().map(|()| cli)) {
            Ok(cli) => launch = Launch { cli, args },
            Err(error) => {
                info!("Forgetting the remembered subdivisions: {error:#}");
                store_subdivisions(guess, guess);
            }
        }
    }
    ensure!(
        launch.cli.subdivisions <= 8,
        "at most 8 subdivisions are supported"
    );
    let given_mode = has_option(&launch.args, "render-mode").then_some(launch.cli.render_mode);
    launch.cli.render_mode =
        viewer::initial_render_mode(given_mode, stored_render_mode().as_deref());
    info!(mode = launch.cli.render_mode.name(), "Render mode");
    let canvas = document()?
        .get_element_by_id(CANVAS_ID)
        .context("no canvas")?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| anyhow!("#{CANVAS_ID} is not a canvas"))?;

    let hidden = watch_hidden()?;
    // Frame histories of closed pages.
    wasm_bindgen_futures::spawn_local(frames::delete_stale());
    let mut running: Option<RunningWorker> = None;
    let mut first = true;
    viewer::run_web(
        canvas,
        launch,
        ui_options(&window, guess),
        Box::new(move |launch, events, wake_up| {
            if let Some(previous) = running.take() {
                previous.stop();
            }
            // The worker always gets the subdivisions, the URL only chosen ones.
            let subdivisions = launch.cli.subdivisions;
            let worker_args = with_default(&launch.args, "subdivisions", &subdivisions.to_string());
            if !std::mem::take(&mut first) {
                replace_query(&args_to_query(&launch.args));
                store_subdivisions(subdivisions, guess);
            }
            match start_worker(
                &args_to_query(&worker_args),
                events.clone(),
                wake_up,
                hidden.clone(),
            ) {
                Ok(worker) => {
                    let control = WorkerControl(worker.worker.clone());
                    running = Some(worker);
                    Box::new(control) as Box<dyn SimulationControl>
                }
                Err(error) => {
                    error!("{error:#}");
                    mark("simulation-failed", &format!("{error:#}"));
                    // Shown in the banner and the panel.
                    let _ = events.send(SimulationEvent::Failed(format!("{error:#}")));
                    Box::new(NoControl)
                }
            }
        }),
        Box::new(|wake| Box::new(frames::IdbStore::open(wake)) as Box<dyn FrameStore>),
        Box::new(remember_render_mode),
        Box::new(LocalStorageSettings),
        // The view shows the state in its banner from its first frame on.
        Box::new(|| {
            show_status("");
            mark("first-frame", "true");
        }),
    )
    .await?;
    Ok(())
}

/// Tracks whether the page was hidden: set when it gets hidden, cleared by whoever reads it.
/// Mobile browsers freeze hidden pages and their workers, so a step that spans such a time says
/// nothing about the speed of the device.
fn watch_hidden() -> anyhow::Result<Rc<Cell<bool>>> {
    let page = document()?;
    let hidden = Rc::new(Cell::new(page.hidden()));
    let flag = hidden.clone();
    let listener = Closure::<dyn FnMut()>::new(move || {
        if document().is_ok_and(|document| document.hidden()) {
            flag.set(true);
        }
    });
    page.add_event_listener_with_callback("visibilitychange", listener.as_ref().unchecked_ref())
        .map_err(|error| anyhow!("watching the visibility of the page: {error:?}"))?;
    // The page lives as long as the listener.
    listener.forget();
    Ok(hidden)
}

/// Replaces the query string of the page URL without reloading the page.
fn replace_query(query: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let path = window.location().pathname().unwrap_or_default();
    let hash = window.location().hash().unwrap_or_default();
    let url = format!("{path}{query}{hash}");
    let result = window
        .history()
        .and_then(|history| history.replace_state_with_url(&JsValue::NULL, "", Some(&url)));
    if let Err(error) = result {
        error!("Could not update the URL: {error:?}");
    }
}

/// A simulation worker with the handlers of its messages, which live as long as it runs.
struct RunningWorker {
    worker: web_sys::Worker,
    _on_message: Closure<dyn FnMut(web_sys::MessageEvent)>,
    _on_error: Closure<dyn FnMut(web_sys::Event)>,
}

impl RunningWorker {
    /// Terminates the worker at once, even in the middle of a step.
    fn stop(self) {
        self.worker.set_onmessage(None);
        self.worker.set_onerror(None);
        self.worker.terminate();
    }
}

/// The simulator's JavaScript module and wasm file that the page loaded, as Trunk's preload links
/// in `web/index.html` name them (relative to the page, with the content hashes of this build).
fn module_files() -> anyhow::Result<(String, String)> {
    let document = document()?;
    let href = |selector: &str| {
        document
            .query_selector(selector)
            .ok()
            .flatten()
            .and_then(|link| link.get_attribute("href"))
            .with_context(|| format!("no `{selector}` in the page"))
    };
    Ok((
        href(r#"link[rel="modulepreload"]"#)?,
        href(r#"link[rel="preload"][type="application/wasm"]"#)?,
    ))
}

/// Starts the simulation worker with the query string `query` and forwards its events to the
/// view.
fn start_worker(
    query: &str,
    events: Sender<SimulationEvent>,
    wake_up: Box<dyn Fn() + Send>,
    hidden: Rc<Cell<bool>>,
) -> anyhow::Result<RunningWorker> {
    let (module, wasm) = module_files()?;
    let options = web_sys::WorkerOptions::new();
    options.set_type(web_sys::WorkerType::Module);
    let url = format!("{WORKER_SCRIPT}{}", worker_query(&module, &wasm, query));
    let worker = web_sys::Worker::new_with_options(&url, &options)
        .map_err(|error| anyhow!("starting the simulation worker: {error:?}"))?;

    let wake_up: Arc<dyn Fn() + Send> = Arc::from(wake_up);
    let mut updates = 0u64;
    let send = {
        let events = events.clone();
        let wake_up = wake_up.clone();
        move |event| {
            if events.send(event).is_ok() {
                wake_up();
            }
        }
    };
    let on_message =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            let data = event.data();
            if let Some(message) = get(&data, "error").as_string() {
                mark("simulation-failed", &message);
                send(SimulationEvent::Failed(message));
                return;
            }
            if get(&data, "generating").is_truthy() {
                send(SimulationEvent::Generating);
                return;
            }
            let (Some(time), Ok(cell_buffer), Ok(plate_buffer)) = (
                get(&data, "time").as_f64(),
                get(&data, "cells").dyn_into::<js_sys::ArrayBuffer>(),
                get(&data, "plates").dyn_into::<js_sys::ArrayBuffer>(),
            ) else {
                error!("Unexpected message from the simulation worker");
                return;
            };
            let cell_words = js_sys::Uint32Array::new(&cell_buffer);
            let words_per_cell = std::mem::size_of::<CellInfo>() / std::mem::size_of::<u32>();
            if !(cell_words.length() as usize).is_multiple_of(words_per_cell) {
                error!(
                    words = cell_words.length(),
                    "Cell attributes of the wrong size from the simulation worker"
                );
                return;
            }
            let cell_count = cell_words.length() as usize / words_per_cell;
            let mut cells: Vec<CellInfo> = vec![bytemuck::Zeroable::zeroed(); cell_count];
            cell_words.copy_to(bytemuck::cast_slice_mut(&mut cells));
            let plate_words = js_sys::Uint32Array::new(&plate_buffer);
            let words_per_plate = std::mem::size_of::<PlateInfo>() / std::mem::size_of::<u32>();
            if !(plate_words.length() as usize).is_multiple_of(words_per_plate) {
                error!("Plates of the wrong size from the simulation worker");
                return;
            }
            let mut plates: Vec<PlateInfo> =
                vec![bytemuck::Zeroable::zeroed(); plate_words.length() as usize / words_per_plate];
            plate_words.copy_to(bytemuck::cast_slice_mut(&mut plates));
            let mesh = match (
                get(&data, "directions").dyn_into::<js_sys::ArrayBuffer>(),
                get(&data, "faces").dyn_into::<js_sys::ArrayBuffer>(),
            ) {
                (Ok(directions), Ok(faces)) => {
                    let floats = js_sys::Float32Array::new(&directions);
                    let mut directions = vec![[0f32; 3]; floats.length() as usize / 3];
                    floats.copy_to(bytemuck::cast_slice_mut(&mut directions));
                    let indices = js_sys::Uint32Array::new(&faces);
                    let mut faces = vec![[0u32; 3]; indices.length() as usize / 3];
                    indices.copy_to(bytemuck::cast_slice_mut(&mut faces));
                    Some(Arc::new(MeshUpdate { directions, faces }))
                }
                _ => None,
            };
            // Not representative if the page is or was hidden since the last update. The mark
            // stays until an update arrives while the page is visible: that one still spans
            // the hidden time.
            let hidden_now = document().is_ok_and(|document| document.hidden());
            let was_hidden = hidden.get() || hidden_now;
            if !hidden_now {
                hidden.set(false);
            }
            let step_seconds = get(&data, "step_seconds").as_f64().filter(|_| !was_hidden);
            let mut fields = Fields::default();
            for layer in Layer::ALL {
                let Ok(buffer) = get(&data, layer.key()).dyn_into::<js_sys::ArrayBuffer>() else {
                    continue;
                };
                let values = js_sys::Float32Array::new(&buffer).to_vec();
                if let Err(message) = fields.insert(layer, values, cell_count) {
                    error!("Field from the simulation worker refused: {message}");
                }
            }
            let frame = get(&data, "frame")
                .dyn_into::<js_sys::ArrayBuffer>()
                .ok()
                .map(|buffer| js_sys::Uint8Array::new(&buffer).to_vec());
            if let Ok(document) = document() {
                document.set_title(&format!("World Simulator — {time:.0} Myr"));
            }
            updates += 1;
            mark("surface-updates", &updates.to_string());
            send(SimulationEvent::Surface(SurfaceUpdate {
                time,
                cells: Arc::new(cells),
                plates: Arc::new(plates),
                frame,
                mesh,
                step_seconds,
                fields,
            }));
        });
    worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    // Uncaught errors in the worker, and a worker script that does not load (a failure to load
    // the module, or of the simulation, arrives as an `{ error }` message, see `web/worker.js`).
    // A script that does not load gives a plain `Event` without a message, not an `ErrorEvent`.
    let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        let detail = get(&event, "message")
            .as_string()
            .filter(|message| !message.is_empty());
        error!("Simulation worker error: {detail:?}");
        // Short, for the banner; the details are in the console.
        let message = detail.unwrap_or_else(|| format!("its worker did not start. {RELOAD_HINT}"));
        mark("simulation-failed", &message);
        if events.send(SimulationEvent::Failed(message)).is_ok() {
            wake_up();
        }
    });
    worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    Ok(RunningWorker {
        worker,
        _on_message: on_message,
        _on_error: on_error,
    })
}

/// The simulation of the worker, between steps.
struct WorkerState {
    scope: web_sys::DedicatedWorkerGlobalScope,
    cli: Cli,
    run: Run,
    /// The layers the page asked for.
    layers: Layers,
    first_update: bool,
    /// Encodes the states into the frames of the page's history.
    encoder: FrameEncoder,
    /// Paused by the page, the steps it asked for meanwhile, and whether the step loop is
    /// stopped (to be scheduled again on resume or a step).
    paused: bool,
    steps: u32,
    stopped: bool,
    /// The next step follows a pause: its time says nothing about the pace.
    after_pause: bool,
    /// The run is complete.
    complete: bool,
}

impl WorkerState {
    /// Sends the current state to the page, with the requested layers.
    fn publish(&mut self, step_time: Option<std::time::Duration>) {
        let planet = &self.run.planet;
        let message = js_sys::Object::new();
        set(&message, "time", &planet.tectonics().time().into());
        let step_time = step_time.filter(|_| !std::mem::take(&mut self.after_pause));
        if let Some(step_time) = step_time {
            set(&message, "step_seconds", &step_time.as_secs_f64().into());
        }
        let transfer = js_sys::Array::new();
        let put_floats = |key: &str, values: &[f32]| {
            let array = js_sys::Float32Array::new_with_length(values.len() as u32);
            array.copy_from(values);
            let buffer = array.buffer();
            set(&message, key, &buffer);
            transfer.push(&buffer);
        };
        for (layer, values) in Fields::extract(planet, self.layers).iter() {
            put_floats(layer.key(), values);
        }
        // The mesh only with the first update.
        let first = std::mem::take(&mut self.first_update);
        let mesh = first.then(|| MeshUpdate::of(planet));
        if let Some(mesh) = &mesh {
            put_floats("directions", bytemuck::cast_slice(&mesh.directions));
        }
        let put_words = |key: &str, values: &[u32]| {
            let array = js_sys::Uint32Array::new_with_length(values.len() as u32);
            array.copy_from(values);
            let buffer = array.buffer();
            set(&message, key, &buffer);
            transfer.push(&buffer);
        };
        let cells = viewer::cell_infos(planet);
        put_words("cells", bytemuck::cast_slice(&cells));
        let plates = viewer::plate_infos(planet);
        put_words("plates", bytemuck::cast_slice(&plates));
        // The frame of the history, here rather than on the page (#174); none for a state
        // sent again.
        let time = planet.tectonics().time();
        if let Some(frame) = self.encoder.encode(time, &cells, &plates) {
            let array = js_sys::Uint8Array::new_with_length(frame.len() as u32);
            array.copy_from(&frame);
            let buffer = array.buffer();
            set(&message, "frame", &buffer);
            transfer.push(&buffer);
        }
        if let Some(mesh) = &mesh {
            put_words("faces", bytemuck::cast_slice(&mesh.faces));
        }
        if let Err(error) = self.scope.post_message_with_transfer(&message, &transfer) {
            error!("Could not send the surface to the page: {error:?}");
            // The page's history did not get this frame: the next must not depend on it.
            self.encoder.force_keyframe();
        }
    }

    /// Takes a request of the page: new layers come with the current state at once.
    fn request(&mut self, layers: Layers) {
        let new = layers.0 & !self.layers.0 != 0;
        self.layers = layers;
        if new {
            let start = web_time::Instant::now();
            self.publish(None);
            // The time of this extra update does not count into the next step's.
            self.run.previous_update += start.elapsed();
        }
    }
}

/// Takes a pause or step request of the page, and schedules the step loop again if it stopped
/// for a pause and may go on.
fn control(state: &Rc<RefCell<WorkerState>>, paused: Option<bool>, step: bool) {
    let restart = {
        let mut worker = state.borrow_mut();
        if let Some(paused) = paused {
            worker.paused = paused;
        }
        if step && worker.paused {
            worker.steps += 1;
        }
        let go = !worker.paused || worker.steps > 0;
        if go && worker.stopped && !worker.complete {
            worker.stopped = false;
            true
        } else {
            false
        }
    };
    if restart {
        schedule_step(state.clone());
    }
}

/// Reports a failure of the simulation to the page.
fn report_failure(scope: &web_sys::DedicatedWorkerGlobalScope, error: &anyhow::Error) {
    error!("Simulation failed: {error:#}");
    let message = js_sys::Object::new();
    set(&message, "error", &format!("{error:#}").into());
    if let Err(error) = scope.post_message(&message) {
        error!("Could not report the error to the page: {error:?}");
    }
}

/// Takes the next step in a later task of the worker, so that messages of the page are handled
/// in between, sends its state, and schedules the step after it.
fn schedule_step(state: Rc<RefCell<WorkerState>>) {
    let scope = state.borrow().scope.clone();
    let callback = Closure::once_into_js(move || {
        let more = {
            let mut guard = state.borrow_mut();
            let worker = &mut *guard;
            // A run paused after its last step still ends (`advance` returns `None`).
            let last = worker.run.step >= worker.run.steps;
            if worker.paused && !last {
                if worker.steps == 0 {
                    // Scheduled again on resume or a step (see `WorkerState::control`).
                    worker.stopped = true;
                    worker.after_pause = true;
                    return;
                }
                worker.steps -= 1;
            }
            match worker.run.advance(&worker.cli) {
                Some(step_time) => {
                    worker.publish(Some(step_time));
                    match worker.run.finish_step(&worker.cli) {
                        Ok(()) => true,
                        Err(error) => {
                            report_failure(&worker.scope, &error);
                            false
                        }
                    }
                }
                None => {
                    worker.run.log_finished();
                    worker.complete = true;
                    false
                }
            }
        };
        if more {
            schedule_step(state);
        }
    });
    if let Err(error) =
        scope.set_timeout_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(), 0)
    {
        error!("Could not schedule the next step: {error:?}");
    }
}

/// Runs the simulation in the worker, sending the surface to the page after every step. The
/// parameters come with its URL, a restart terminates it and starts a new worker; the messages
/// from the page are requests for other fields (`{ layers }`), to pause or resume
/// (`{ paused }`) and for one step while paused (`{ step }`), handled between steps.
fn run_worker(scope: &web_sys::DedicatedWorkerGlobalScope) -> anyhow::Result<()> {
    let search = without_worker_files(&scope.location().search());
    let Launch { cli, .. } = parse_query(&search)?;
    info!(?search, "Simulation worker started");
    let message = js_sys::Object::new();
    set(&message, "generating", &true.into());
    if let Err(error) = scope.post_message(&message) {
        error!("Could not send the state to the page: {error:?}");
    }
    let planet = build_planet(&cli)?;
    let run = Run::new(planet, &cli)?;
    let layers = cli.render_mode.layers();
    let state = Rc::new(RefCell::new(WorkerState {
        scope: scope.clone(),
        cli,
        run,
        layers,
        first_update: true,
        encoder: FrameEncoder::default(),
        paused: false,
        steps: 0,
        stopped: false,
        after_pause: false,
        complete: false,
    }));
    state.borrow_mut().publish(None);
    let requests = state.clone();
    let on_message =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            let data = event.data();
            if let Some(bits) = get(&data, "layers").as_f64() {
                requests.borrow_mut().request(Layers(bits as u32));
            } else if let Some(paused) = get(&data, "paused").as_bool() {
                control(&requests, Some(paused), false);
            } else if get(&data, "step").is_truthy() {
                control(&requests, None, true);
            } else {
                error!("Unexpected message from the page");
            }
        });
    scope.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    // The worker lives as long as the handler.
    on_message.forget();
    schedule_step(state);
    Ok(())
}
