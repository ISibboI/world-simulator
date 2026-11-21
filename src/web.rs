//! The web version (`wasm32`): the page shows the same 3D view as the desktop window, and the
//! simulation runs in a Web Worker so that the view stays responsive while a step runs.
//!
//! Both are the same wasm module: `web/index.html` loads it on the page, and `web/worker.js`
//! loads it again in a module worker. [`start`] tells them apart by their global scope. The
//! parameters are the command-line options, given as URL query parameters
//! (`?seed=3&subdivisions=5&no-erosion&set=basal_drag=8e14`, see [`crate::query`]); the worker
//! gets the same query string. It sends a surface update after every step as
//! `{ time, vertices, cells }` (the vertices, the attributes of the cells as
//! [`viewer::CellInfo`]s, and with the first update also `faces`, the triangles), with the buffers
//! transferred rather than copied, or `{ error }` if the simulation fails. A restart from the
//! parameter panel terminates the worker and starts a new one with the new query string, which
//! also replaces the query of the page URL, so that the setting can be shared as a link. Each
//! update after the first also carries `step_seconds`, the time of the step in the worker.
//!
//! Without `subdivisions` in the URL, the page starts with the subdivisions remembered in the
//! local storage from the last restart, or else with a guess for the device (see
//! [`crate::device`]). The worker always gets them explicitly; the page URL only when the user
//! chose them.

use std::cell::Cell;
use std::ops::ControlFlow;
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
use crate::planet::Planet;
use crate::query::{args_to_query, query_to_args};
use crate::viewer::{
    self, CellInfo, Launch, SimulationEvent, SurfaceUpdate, UiOptions, Vertex, has_option,
    with_default,
};
use crate::{Cli, build_planet, simulate};

/// Id of the canvas in `web/index.html`.
const CANVAS_ID: &str = "world_simulator";
/// Id of the element in `web/index.html` that shows the loading state and errors.
const STATUS_ID: &str = "status";
/// Script of the simulation worker, relative to the page.
const WORKER_SCRIPT: &str = "./worker.js";

/// Entry point of the page and of the worker.
pub fn start() {
    console_error_panic_hook::set_once();
    init_logging();
    let global = js_sys::global();
    if let Some(scope) = global.dyn_ref::<web_sys::DedicatedWorkerGlobalScope>() {
        if let Err(error) = run_worker(scope) {
            error!("Simulation failed: {error:#}");
            let message = js_sys::Object::new();
            set(&message, "error", &format!("{error:#}").into());
            if let Err(error) = scope.post_message(&message) {
                error!("Could not report the error to the page: {error:?}");
            }
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

/// Starts the 3D view on the page and the simulation worker.
async fn run_page() -> anyhow::Result<()> {
    let window = web_sys::window().context("no window")?;
    let search = window
        .location()
        .search()
        .map_err(|error| anyhow!("reading the URL: {error:?}"))?;
    let url_args = query_to_args(&search)?;
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
    let canvas = document()?
        .get_element_by_id(CANVAS_ID)
        .context("no canvas")?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| anyhow!("#{CANVAS_ID} is not a canvas"))?;

    show_status("Starting the simulation…");
    let hidden = watch_hidden()?;
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
                show_status("");
                replace_query(&args_to_query(&launch.args));
                store_subdivisions(subdivisions, guess);
            }
            match start_worker(
                &args_to_query(&worker_args),
                events.clone(),
                wake_up,
                hidden.clone(),
            ) {
                Ok(worker) => running = Some(worker),
                Err(error) => {
                    error!("{error:#}");
                    show_status(&format!("{error:#}"));
                    // The panel shows it too (the view repaints on its next input).
                    let _ = events.send(SimulationEvent::Failed(format!("{error:#}")));
                }
            }
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
    _on_error: Closure<dyn FnMut(web_sys::ErrorEvent)>,
}

impl RunningWorker {
    /// Terminates the worker at once, even in the middle of a step.
    fn stop(self) {
        self.worker.set_onmessage(None);
        self.worker.set_onerror(None);
        self.worker.terminate();
    }
}

/// Starts the simulation worker with the query string `query` and forwards its events to the
/// view.
fn start_worker(
    query: &str,
    events: Sender<SimulationEvent>,
    wake_up: Box<dyn Fn() + Send>,
    hidden: Rc<Cell<bool>>,
) -> anyhow::Result<RunningWorker> {
    let options = web_sys::WorkerOptions::new();
    options.set_type(web_sys::WorkerType::Module);
    let worker = web_sys::Worker::new_with_options(&format!("{WORKER_SCRIPT}{query}"), &options)
        .map_err(|error| anyhow!("starting the simulation worker: {error:?}"))?;

    let wake_up: Arc<dyn Fn() + Send> = Arc::from(wake_up);
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
                show_status(&format!("Simulation failed: {message}"));
                send(SimulationEvent::Failed(message));
                return;
            }
            let (Some(time), Ok(buffer), Ok(cell_buffer)) = (
                get(&data, "time").as_f64(),
                get(&data, "vertices").dyn_into::<js_sys::ArrayBuffer>(),
                get(&data, "cells").dyn_into::<js_sys::ArrayBuffer>(),
            ) else {
                error!("Unexpected message from the simulation worker");
                return;
            };
            let floats = js_sys::Float32Array::new(&buffer);
            let floats_per_vertex = std::mem::size_of::<Vertex>() / std::mem::size_of::<f32>();
            if !(floats.length() as usize).is_multiple_of(floats_per_vertex) {
                error!(
                    floats = floats.length(),
                    "Surface update of the wrong size from the simulation worker"
                );
                return;
            }
            let vertex_count = floats.length() as usize / floats_per_vertex;
            let mut vertices: Vec<Vertex> = vec![bytemuck::Zeroable::zeroed(); vertex_count];
            floats.copy_to(bytemuck::cast_slice_mut(&mut vertices));
            let cell_words = js_sys::Uint32Array::new(&cell_buffer);
            let words_per_cell = std::mem::size_of::<CellInfo>() / std::mem::size_of::<u32>();
            if cell_words.length() as usize != vertex_count * words_per_cell {
                error!(
                    words = cell_words.length(),
                    vertex_count, "Cell attributes of the wrong size from the simulation worker"
                );
                return;
            }
            let mut cells: Vec<CellInfo> = vec![bytemuck::Zeroable::zeroed(); vertex_count];
            cell_words.copy_to(bytemuck::cast_slice_mut(&mut cells));
            let faces = get(&data, "faces")
                .dyn_into::<js_sys::ArrayBuffer>()
                .ok()
                .map(|buffer| {
                    let indices = js_sys::Uint32Array::new(&buffer);
                    let mut faces = vec![[0u32; 3]; indices.length() as usize / 3];
                    indices.copy_to(bytemuck::cast_slice_mut(&mut faces));
                    Arc::new(faces)
                });
            if time == 0.0 {
                show_status("");
            }
            // Not representative if the page is or was hidden since the last update. The mark
            // stays until an update arrives while the page is visible: that one still spans
            // the hidden time.
            let hidden_now = document().is_ok_and(|document| document.hidden());
            let was_hidden = hidden.get() || hidden_now;
            if !hidden_now {
                hidden.set(false);
            }
            let step_seconds = get(&data, "step_seconds").as_f64().filter(|_| !was_hidden);
            if let Ok(document) = document() {
                document.set_title(&format!("World Simulator — {time:.0} Myr"));
            }
            send(SimulationEvent::Surface(SurfaceUpdate {
                time,
                vertices: Arc::new(vertices),
                cells: Arc::new(cells),
                faces,
                step_seconds,
            }));
        });
    worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    // Errors while loading the worker script or module (a failure inside the simulation arrives
    // as an `{ error }` message, see `web/worker.js`).
    let on_error =
        Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(move |event: web_sys::ErrorEvent| {
            let message = event.message();
            error!("Simulation worker error: {message}");
            show_status(&format!("Simulation failed: {message}"));
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

/// Runs the whole simulation in the worker, sending the surface to the page after every step.
/// The loop blocks the worker, which needs no messages from the page: the parameters come
/// with its URL, and a restart terminates it and starts a new worker.
fn run_worker(scope: &web_sys::DedicatedWorkerGlobalScope) -> anyhow::Result<()> {
    let search = scope.location().search();
    let Launch { cli, .. } = parse_query(&search)?;
    info!(?search, "Simulation worker started");
    let planet = build_planet(&cli)?;
    let mut first_update = true;
    let publish = |planet: &Planet, step_time: Option<std::time::Duration>| {
        let vertices = viewer::surface_vertices(planet, cli.exaggeration, cli.river_threshold);
        let floats: &[f32] = bytemuck::cast_slice(&vertices);
        let array = js_sys::Float32Array::new_with_length(floats.len() as u32);
        array.copy_from(floats);
        let buffer = array.buffer();
        let message = js_sys::Object::new();
        set(&message, "time", &planet.tectonics().time().into());
        if let Some(step_time) = step_time {
            set(&message, "step_seconds", &step_time.as_secs_f64().into());
        }
        set(&message, "vertices", &buffer);
        let cells = viewer::cell_infos(planet);
        let words: &[u32] = bytemuck::cast_slice(&cells);
        let array = js_sys::Uint32Array::new_with_length(words.len() as u32);
        array.copy_from(words);
        let cell_buffer = array.buffer();
        set(&message, "cells", &cell_buffer);
        let transfer = js_sys::Array::of2(&buffer, &cell_buffer);
        // The triangles only with the first update.
        if std::mem::take(&mut first_update) {
            let indices: &[u32] = bytemuck::cast_slice(planet.tectonics().mesh().faces());
            let array = js_sys::Uint32Array::new_with_length(indices.len() as u32);
            array.copy_from(indices);
            let faces = array.buffer();
            set(&message, "faces", &faces);
            transfer.push(&faces);
        }
        if let Err(error) = scope.post_message_with_transfer(&message, &transfer) {
            error!("Could not send the surface to the page: {error:?}");
        }
        ControlFlow::Continue(())
    };
    simulate(planet, &cli, publish)
}
