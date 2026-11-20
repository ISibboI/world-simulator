//! The web version (`wasm32`): the page shows the same 3D view as the desktop window, and the
//! simulation runs in a Web Worker so that the view stays responsive while a step runs.
//!
//! Both are the same wasm module: `web/index.html` loads it on the page, and `web/worker.js`
//! loads it again in a module worker. [`start`] tells them apart by their global scope. The
//! parameters are the command-line options, given as URL query parameters
//! (`?seed=3&subdivisions=5&no-erosion&set=basal_drag=8e14`); the worker gets the same query
//! string. It sends a surface update after every step as `{ time, vertices }`, with the vertex
//! buffer transferred rather than copied, or `{ error }` if the simulation fails.

use std::sync::Arc;
use std::sync::mpsc::Sender;

use anyhow::{Context, anyhow, ensure};
use clap::Parser;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

use tracing::{error, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::planet::Planet;
use crate::sphere::SphereMesh;
use crate::viewer::{self, SurfaceUpdate, Vertex};
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

/// Parses the query string of the page (or worker) URL into the command-line options.
fn parse_query(search: &str) -> anyhow::Result<Cli> {
    let mut args = vec!["world-simulator".to_owned()];
    for pair in search
        .trim_start_matches('?')
        .split('&')
        .filter(|pair| !pair.is_empty())
    {
        let (name, value) = match pair.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (pair, None),
        };
        let name = decode(name)?.replace('_', "-");
        ensure!(
            !matches!(
                name.as_str(),
                "headless" | "output" | "width" | "cell-dump" | "help" | "version"
            ),
            "the option `{name}` is not available on the web"
        );
        // One `--name=value` token, so that values starting with `-` are not taken as options.
        match value.filter(|value| !value.is_empty()) {
            Some(value) => args.push(format!("--{name}={}", decode(value)?)),
            None => args.push(format!("--{name}")),
        }
    }
    Cli::try_parse_from(args).map_err(|error| anyhow!("invalid URL parameters: {error}"))
}

/// Decodes a percent-encoded query component (`+` is a space).
fn decode(component: &str) -> anyhow::Result<String> {
    js_sys::decode_uri_component(&component.replace('+', " "))
        .map(String::from)
        .map_err(|_| anyhow!("invalid percent-encoding in the URL: {component}"))
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

/// Starts the 3D view on the page and the simulation worker.
async fn run_page() -> anyhow::Result<()> {
    let window = web_sys::window().context("no window")?;
    let search = window
        .location()
        .search()
        .map_err(|error| anyhow!("reading the URL: {error:?}"))?;
    let cli = parse_query(&search)?;
    ensure!(
        cli.subdivisions <= 8,
        "at most 8 subdivisions are supported"
    );
    let canvas = document()?
        .get_element_by_id(CANVAS_ID)
        .context("no canvas")?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| anyhow!("#{CANVAS_ID} is not a canvas"))?;

    // The mesh is deterministic, so the page builds its own copy for the triangles instead of
    // receiving them from the worker.
    let mesh = SphereMesh::icosphere(cli.subdivisions);
    let (vertex_count, faces) = (mesh.len(), mesh.faces().to_vec());
    show_status("Starting the simulation…");
    viewer::run_web(
        canvas,
        vertex_count,
        faces,
        Box::new(move |updates, wake_up| {
            if let Err(error) = start_worker(&search, vertex_count, updates, wake_up) {
                error!("{error:#}");
                show_status(&format!("{error:#}"));
            }
        }),
    )
    .await?;
    Ok(())
}

/// Starts the simulation worker with the page's query string and forwards its surface updates
/// to the view.
fn start_worker(
    search: &str,
    vertex_count: usize,
    updates: Sender<SurfaceUpdate>,
    wake_up: Box<dyn Fn() + Send>,
) -> anyhow::Result<()> {
    let options = web_sys::WorkerOptions::new();
    options.set_type(web_sys::WorkerType::Module);
    let worker = web_sys::Worker::new_with_options(&format!("{WORKER_SCRIPT}{search}"), &options)
        .map_err(|error| anyhow!("starting the simulation worker: {error:?}"))?;

    let on_message =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            let data = event.data();
            if let Some(message) = get(&data, "error").as_string() {
                show_status(&format!("Simulation failed: {message}"));
                return;
            }
            let (Some(time), Ok(buffer)) = (
                get(&data, "time").as_f64(),
                get(&data, "vertices").dyn_into::<js_sys::ArrayBuffer>(),
            ) else {
                error!("Unexpected message from the simulation worker");
                return;
            };
            let floats = js_sys::Float32Array::new(&buffer);
            let floats_per_vertex = std::mem::size_of::<Vertex>() / std::mem::size_of::<f32>();
            if floats.length() as usize != vertex_count * floats_per_vertex {
                error!(
                    floats = floats.length(),
                    vertex_count, "Surface update of the wrong size from the simulation worker"
                );
                return;
            }
            let mut vertices: Vec<Vertex> = vec![bytemuck::Zeroable::zeroed(); vertex_count];
            floats.copy_to(bytemuck::cast_slice_mut(&mut vertices));
            if time == 0.0 {
                show_status("");
            }
            if let Ok(document) = document() {
                document.set_title(&format!("World Simulator — {time:.0} Myr"));
            }
            if updates
                .send(SurfaceUpdate {
                    time,
                    vertices: Arc::new(vertices),
                })
                .is_ok()
            {
                wake_up();
            }
        });
    worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    // Errors while loading the worker script or module (a failure inside the simulation arrives
    // as an `{ error }` message, see `web/worker.js`).
    let on_error = Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(|event: web_sys::ErrorEvent| {
        let message = event.message();
        error!("Simulation worker error: {message}");
        show_status(&format!("Simulation failed: {message}"));
    });
    worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    // The worker and its handlers live as long as the page.
    on_message.forget();
    on_error.forget();
    std::mem::forget(worker);
    Ok(())
}

/// Runs the whole simulation in the worker, sending the surface to the page after every step.
/// The loop blocks the worker, which needs no messages from the page: the parameters come
/// with its URL, and a restart would start a new worker.
fn run_worker(scope: &web_sys::DedicatedWorkerGlobalScope) -> anyhow::Result<()> {
    let search = scope.location().search();
    let cli = parse_query(&search)?;
    info!(?search, "Simulation worker started");
    let planet = build_planet(&cli)?;
    let publish = |planet: &Planet| {
        let vertices = viewer::surface_vertices(planet, cli.exaggeration, cli.river_threshold);
        let floats: &[f32] = bytemuck::cast_slice(&vertices);
        let array = js_sys::Float32Array::new_with_length(floats.len() as u32);
        array.copy_from(floats);
        let buffer = array.buffer();
        let message = js_sys::Object::new();
        set(&message, "time", &planet.tectonics().time().into());
        set(&message, "vertices", &buffer);
        if let Err(error) = scope.post_message_with_transfer(&message, &js_sys::Array::of1(&buffer))
        {
            error!("Could not send the surface to the page: {error:?}");
        }
    };
    simulate(planet, &cli, publish)
}
