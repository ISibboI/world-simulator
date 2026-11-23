// Runs the simulation in a Web Worker: the same wasm module as the page. Its entry point
// (`web::start` in src/web.rs) sees the worker scope and runs the simulation with the parameters
// in this script's query string. A failure (e.g. a panic) is reported to the page.
//
// The page names the files of its build to load, `module` (the JavaScript module) and `wasm`, at
// the start of the query string (see `worker_query` in src/query.rs). Their names carry content
// hashes, and the query string makes this script's URL unique per build of the module, so that the
// page and the worker never combine files of different builds from the browser cache (issue
// #122). (A build that changes only this script keeps its URL: a page may get the cached script
// of the previous build for a while, so keep this script compatible with the page's messages
// and query string.)
//
// Messages from the page (requests of other fields, `{ layers }`) that arrive while the module
// is still loading would find no handler: they are kept and handed to the handler that the
// simulation installs, once it runs.

const early = [];
self.onmessage = (event) => early.push(event.data);

// A file of the page's build: only from the site itself.
const file = (name) => {
    const given = new URLSearchParams(location.search).get(name);
    if (!given) {
        throw new Error(`no ${name} file given: the page and this script are of different builds`);
    }
    const url = new URL(given, location.href);
    if (url.origin !== location.origin) {
        throw new Error(`the ${name} file is not on this site: ${url}`);
    }
    return url.href;
};

// Failures to get or compile the files, as opposed to failures (traps, e.g. a panic) of the
// simulation itself.
const loading = (error) =>
    !(typeof WebAssembly === "object" && error instanceof WebAssembly.RuntimeError);

(async () => {
    const { default: init } = await import(file("module"));
    await init({ module_or_path: file("wasm") });
    for (const data of early.splice(0)) {
        self.dispatchEvent(new MessageEvent("message", { data }));
    }
})().catch((error) => {
    console.error("The simulation worker failed:", error);
    // Short, for the banner; the details are in the console.
    postMessage({
        error: loading(error)
            ? "the worker could not load the simulator. Reload the page."
            : String(error?.message ?? error),
    });
});
