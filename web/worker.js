// Runs the simulation in a Web Worker: the same wasm module as the page. Its entry point
// (`web::start` in src/web.rs) sees the worker scope and runs the simulation with the parameters
// in this script's query string. A failure (e.g. a panic) is reported to the page.
import init from "./world-simulator.js";

init().catch((error) => {
    postMessage({ error: String(error?.message ?? error) });
});
