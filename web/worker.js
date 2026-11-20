// Runs the simulation in a Web Worker: the same wasm module as the page. Its entry point
// (`web::start` in src/web.rs) sees the worker scope and runs the simulation with the parameters
// in this script's query string.
import init from "./world-simulator.js";

init();
