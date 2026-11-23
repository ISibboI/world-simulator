// Injected by browser_test.py as the first script of the page under test (never part of the
// site). Collects the page's console output and uncaught errors, and reports them with the state
// the page records on its root element (`data-*`, see `mark` in src/web.rs) to the test server
// every half second, until the server says it has seen enough. The test passes or fails on the
// server, so that it works the same with every way of opening the page (WebDriver, Playwright,
// an Android intent, the iOS Simulator).
(() => {
    const logs = [];
    const errors = [];
    const started = Date.now();
    const keep = (list, text) => {
        if (list.length < 2000) {
            list.push(`${((Date.now() - started) / 1000).toFixed(1)} s ${text}`);
        }
    };
    for (const level of ["log", "info", "warn", "error", "debug"]) {
        const original = console[level].bind(console);
        console[level] = (...args) => {
            keep(logs, `${level}: ${args.map(String).join(" ").slice(0, 2000)}`);
            original(...args);
        };
    }
    addEventListener("error", (event) =>
        keep(errors, `error: ${event.message} (${event.filename}:${event.lineno})`));
    addEventListener("unhandledrejection", (event) =>
        keep(errors, `unhandled rejection: ${event.reason?.stack ?? event.reason}`));

    // Changes of the page state, and stalls of the page (no animation frame for over a second,
    // e.g. a long frame of the view), into the log with their time.
    let previous = "";
    const watch = () => {
        // Not every new state from the worker, only the first ones.
        const { surfaceUpdates, ...rest } = document.documentElement.dataset;
        const key = JSON.stringify(rest) + (Number(surfaceUpdates ?? 0) < 3 ? surfaceUpdates : "");
        if (key !== previous) {
            keep(logs, `state: ${JSON.stringify(document.documentElement.dataset)}`);
            previous = key;
        }
    };
    let lastFrame = performance.now();
    const frame = (now) => {
        if (now - lastFrame > 1000 && !document.hidden) {
            keep(logs, `stall: no animation frame for ${((now - lastFrame) / 1000).toFixed(1)} s`);
        }
        lastFrame = now;
        watch();
        requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);
    addEventListener("visibilitychange", () => keep(logs, `visibility: ${document.visibilityState}`));

    let sent = 0;
    const report = async () => {
        watch();
        const status = document.getElementById("status");
        const body = {
            seconds: (Date.now() - started) / 1000,
            hash: location.hash,
            userAgent: navigator.userAgent,
            data: { ...document.documentElement.dataset },
            status: status ? { text: status.textContent, display: status.style.display } : null,
            title: document.title,
            webLocks: "locks" in navigator,
            databases: typeof indexedDB?.databases === "function",
            logs: logs.slice(sent),
            errors,
        };
        try {
            const response = await fetch("/report", { method: "POST", body: JSON.stringify(body) });
            sent += body.logs.length;
            const answer = await response.json();
            if (answer.stop) {
                return;
            }
        } catch (error) {
            // The server may be gone; try again.
        }
        setTimeout(report, 500);
    };
    setTimeout(report, 500);
})();
