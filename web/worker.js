// Runs the simulation in a Web Worker: the same wasm module as the page. Its entry point
// (`web::start` in src/web.rs) sees the worker scope and runs the simulation with the parameters
// in this script's query string. A failure (e.g. a panic) is reported to the page.
//
// Messages from the page (requests of other fields, `{ layers }`) that arrive while the module
// is still loading would find no handler: they are kept and handed to the handler that the
// simulation installs, once it runs.
import init from "./world-simulator.js";

const early = [];
self.onmessage = (event) => early.push(event.data);

init()
    .then(() => {
        for (const data of early.splice(0)) {
            self.dispatchEvent(new MessageEvent("message", { data }));
        }
    })
    .catch((error) => {
        postMessage({ error: String(error?.message ?? error) });
    });
