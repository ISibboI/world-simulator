#!/usr/bin/env python3
"""Basic browser test of the web version (issue #117): the simulator loads, the 3D view draws a
frame, and the frame history is stored in IndexedDB and read back.

It serves the built site (`trunk build --release`, `dist/`) on localhost with `probe.js` injected
as the first script of the page, opens the page in the browser under test, and waits for the
probe's reports of the page state (the `data-*` attributes of the root element, see `mark` in
src/web.rs, plus console output and uncaught errors). It passes when

- the loading banner is gone and the view drew its first frame (`data-first-frame`),
- the simulation worker sent at least `--min-updates` states (`data-surface-updates`),
- the frame history read its first frame back from IndexedDB (`data-frame-history=indexeddb`),
- nothing failed (`data-simulation-failed`, uncaught errors),

and a screenshot shows the planet: enough ocean-blue pixels in the middle of the screen (the
runners have no GPU, so this is software rendering everywhere).

Browsers (`--browser`):
  chrome, firefox, safari, edge   the installed browser through Selenium (WebDriver)
  webkit, chromium-pw, firefox-pw Playwright's browsers
  android-chrome, android-firefox the browser in a running Android emulator (adb); Firefox needs
                                  `--apk` (Fenix)
  ios-safari                      Mobile Safari in an iOS Simulator (xcrun simctl)

Results go to `--out` (result.json, page.log, screenshot.png). Requirements: Python 3.9+,
Pillow, and selenium or playwright for those browsers.
"""

import argparse
import functools
import http.server
import io
import json
import os
import subprocess
import sys
import threading
import time
from pathlib import Path

TESTS = Path(__file__).resolve().parent


class State:
    """The reports of the probe, shared with the server threads."""

    def __init__(self, token):
        self.token = token
        self.lock = threading.Lock()
        self.last = None
        self.logs = []
        self.reports = 0
        self.stop = False

    def add(self, report):
        if report.get("hash") != f"#{self.token}":
            # A page of an earlier test, restored by the browser.
            return True
        with self.lock:
            self.logs.extend(report.pop("logs", []))
            self.last = report
            self.reports += 1
            return self.stop


def make_handler(dist, state):
    class Handler(http.server.SimpleHTTPRequestHandler):
        extensions_map = {
            **http.server.SimpleHTTPRequestHandler.extensions_map,
            ".wasm": "application/wasm",
            ".js": "text/javascript",
        }

        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(dist), **kwargs)

        def log_message(self, *args):
            pass

        def end_headers(self):
            self.send_header("Cache-Control", "no-store")
            super().end_headers()

        def send_bytes(self, body, content_type):
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            path = self.path.split("?")[0].split("#")[0]
            if path in ("/", "/index.html"):
                page = (dist / "index.html").read_text(encoding="utf-8")
                page = page.replace("<head>", '<head>\n<script src="/probe.js"></script>', 1)
                self.send_bytes(page.encode(), "text/html; charset=utf-8")
            elif path == "/probe.js":
                self.send_bytes((TESTS / "probe.js").read_bytes(), "text/javascript")
            else:
                super().do_GET()

        def do_POST(self):
            if self.path != "/report":
                self.send_error(404)
                return
            length = int(self.headers.get("Content-Length", 0))
            try:
                report = json.loads(self.rfile.read(length))
            except ValueError:
                self.send_error(400)
                return
            stop = state.add(report)
            self.send_bytes(json.dumps({"stop": stop}).encode(), "application/json")

    return Handler


def run(command, check=True, capture=False, timeout=120):
    print("+", " ".join(command), flush=True)
    result = subprocess.run(
        command, check=check, timeout=timeout, capture_output=capture, text=not capture
    )
    return result


# The browsers: each opens `url` and returns (screenshot function, close function).


def open_selenium(name, url):
    from selenium import webdriver

    if name == "chrome" or name == "edge":
        options = webdriver.ChromeOptions() if name == "chrome" else webdriver.EdgeOptions()
        for argument in [
            # No GPU on the runners: allow software WebGL.
            "--ignore-gpu-blocklist",
            "--enable-unsafe-swiftshader",
            "--no-first-run",
            "--no-default-browser-check",
            "--window-size=1280,800",
        ]:
            options.add_argument(argument)
        for argument in os.environ.get("BROWSER_ARGS", "").split():
            options.add_argument(argument)
        driver = webdriver.Chrome(options=options) if name == "chrome" else webdriver.Edge(options=options)
    elif name == "firefox":
        options = webdriver.FirefoxOptions()
        # Software rendering is on Firefox's blocklist for WebGL.
        options.set_preference("webgl.force-enabled", True)
        driver = webdriver.Firefox(options=options)
        driver.set_window_size(1280, 800)
    elif name == "safari":
        driver = webdriver.Safari()
        driver.set_window_size(1280, 800)
    else:
        raise ValueError(name)
    print("browser:", driver.capabilities.get("browserName"), driver.capabilities.get("browserVersion"))
    driver.get(url)
    return driver.get_screenshot_as_png, driver.quit, None


def open_playwright(name, url):
    from playwright.sync_api import sync_playwright

    playwright = sync_playwright().start()
    kind = {"webkit": "webkit", "chromium-pw": "chromium", "firefox-pw": "firefox"}[name]
    launch = {"headless": not os.environ.get("DISPLAY")}
    if kind == "chromium":
        launch["args"] = ["--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"]
    if kind == "firefox":
        launch["firefox_user_prefs"] = {"webgl.force-enabled": True}
    browser = getattr(playwright, kind).launch(**launch)
    print("browser:", kind, browser.version)
    page = browser.new_page(viewport={"width": 1280, "height": 800})
    page.goto(url)

    def close():
        browser.close()
        playwright.stop()

    return page.screenshot, close, None


ANDROID_CHROME = "com.android.chrome"
ANDROID_FIREFOX = "org.mozilla.firefox"


def adb_shell(command, check=True):
    return run(["adb", "shell", command], check=check)


# Buttons of first-run screens and prompts that cover the page (Firefox's onboarding, Chrome's
# first-run and notification prompts), tapped away while the test waits.
DISMISS = (
    "continue", "not now", "skip", "no thanks", "no, thanks", "maybe later", "got it",
    "accept & continue", "use without an account", "dismiss",
)


def dismiss_android_prompts():
    import re
    import xml.etree.ElementTree as ElementTree

    run(["adb", "shell", "uiautomator", "dump", "/sdcard/window.xml"], check=False, timeout=60)
    dump = subprocess.run(
        ["adb", "exec-out", "cat", "/sdcard/window.xml"], capture_output=True, text=True, timeout=60
    ).stdout
    try:
        root = ElementTree.fromstring(dump[dump.find("<?xml") :])
    except ElementTree.ParseError as error:
        print("no window dump:", error, dump[:200], flush=True)
        return
    labels = []
    for node in root.iter("node"):
        label = (node.get("text") or node.get("content-desc") or "").strip()
        if label:
            labels.append(label)
        if label.lower() in DISMISS:
            numbers = [int(n) for n in re.findall(r"\d+", node.get("bounds", ""))]
            if len(numbers) == 4:
                x, y = (numbers[0] + numbers[2]) // 2, (numbers[1] + numbers[3]) // 2
                print(f"tapping {label!r} at {x},{y}", flush=True)
                run(["adb", "shell", "input", "tap", str(x), str(y)], check=False)
                return
    print("on the screen:", labels[:30], flush=True)


def open_android(name, url, port, apk):
    run(["adb", "reverse", f"tcp:{port}", f"tcp:{port}"])
    run(["adb", "shell", "getprop", "ro.build.version.release"], check=False)
    if name == "android-chrome":
        package = ANDROID_CHROME
        # Command-line flags for Chrome (read because it is the debug app): no first-run
        # screens, and WebGL on the emulator's software GPU.
        adb_shell(f"am set-debug-app --persistent {package}")
        flags = "_ --disable-fre --no-default-browser-check --no-first-run --ignore-gpu-blocklist --enable-unsafe-swiftshader"
        adb_shell(f"echo '{flags}' > /data/local/tmp/chrome-command-line")
        adb_shell(f"am force-stop {package}", check=False)
        activity = f"{package}/com.google.android.apps.chrome.Main"
    else:
        package = ANDROID_FIREFOX
        if apk:
            run(["adb", "install", "-r", str(apk)], timeout=300)
        # GeckoView reads its prefs from this file when the app is the debug app.
        adb_shell(f"am set-debug-app --persistent {package}")
        adb_shell(
            "printf 'prefs:\\n  webgl.force-enabled: true\\n' > "
            f"/data/local/tmp/{package}-geckoview-config.yaml"
        )
        adb_shell(f"am force-stop {package}", check=False)
        activity = f"{package}/org.mozilla.fenix.IntentReceiverActivity"
    run(["adb", "shell", "dumpsys", "package", package, "|", "grep", "versionName"], check=False)
    adb_shell(f"am start -a android.intent.action.VIEW -d '{url}' -n {activity}")

    def screenshot():
        return run(["adb", "exec-out", "screencap", "-p"], capture=True).stdout

    def close():
        adb_shell(f"am force-stop {package}", check=False)

    return screenshot, close, dismiss_android_prompts


def open_ios(url):
    listing = json.loads(run(["xcrun", "simctl", "list", "devices", "available", "-j"], capture=True).stdout)
    candidates = []
    for runtime, devices in listing["devices"].items():
        if "iOS" not in runtime:
            continue
        version = tuple(int(part) for part in runtime.rsplit("iOS-", 1)[-1].split("-") if part.isdigit())
        for device in devices:
            if device["name"].startswith("iPhone"):
                candidates.append((version, device["name"], device["udid"]))
    if not candidates:
        raise RuntimeError("no iPhone simulator available")
    # The newest iOS; of its iPhones the one sorting first by name.
    newest = max(candidate[0] for candidate in candidates)
    version, device_name, udid = sorted(c for c in candidates if c[0] == newest)[0]
    print("simulator:", device_name, "iOS", ".".join(map(str, version)), udid)
    # Booted once; a later test reuses it.
    run(["xcrun", "simctl", "boot", udid], check=False)
    run(["xcrun", "simctl", "bootstatus", udid, "-b"], timeout=900)
    for attempt in range(5):
        if run(["xcrun", "simctl", "openurl", udid, url], check=False, timeout=180).returncode == 0:
            break
        time.sleep(10)
    else:
        raise RuntimeError("the simulator did not open the page")
    folder = Path(os.environ.get("TMPDIR", ".")).resolve()

    def screenshot():
        path = folder / "ios-screenshot.png"
        run(["xcrun", "simctl", "io", udid, "screenshot", str(path)])
        return path.read_bytes()

    def close():
        run(["xcrun", "simctl", "terminate", udid, "com.apple.mobilesafari"], check=False)

    return screenshot, close, None


def check_state(report, min_updates, loaded_only=False):
    """Returns (done, failure): done when the page passed or failed for good."""
    data = report.get("data", {})
    if loaded_only:
        # Builds without the page state (before #117): the banner is gone and the title shows
        # the simulated time.
        if report.get("errors"):
            return True, "uncaught errors: " + "; ".join(report["errors"][:5])
        status = report.get("status") or {}
        return status.get("display") == "none" and "Myr" in report.get("title", ""), None
    if report.get("errors"):
        return True, "uncaught errors: " + "; ".join(report["errors"][:5])
    if "simulationFailed" in data:
        return True, "simulation failed: " + data["simulationFailed"]
    status = report.get("status") or {}
    if status.get("display") != "none" and "could not" in (status.get("text") or ""):
        return True, "error banner: " + status["text"]
    if data.get("frameHistory") == "memory":
        return True, "the frame history fell back to memory (IndexedDB did not work)"
    updates = int(data.get("surfaceUpdates", "0"))
    passed = (
        data.get("firstFrame") == "true"
        and status.get("display") == "none"
        and updates >= min_updates
        and data.get("frameHistory") == "indexeddb"
    )
    return passed, None


def describe(report):
    if report is None:
        return "no report from the page"
    data = report.get("data", {})
    status = report.get("status") or {}
    return (
        f"{report.get('seconds', 0):.0f} s: data {data}, banner "
        f"{status.get('display') or 'shown'} {status.get('text')!r}"
    )


def planet_share(png):
    """The share of ocean-blue pixels in the middle of the screenshot (the planet's place)."""
    from PIL import Image

    image = Image.open(io.BytesIO(png)).convert("RGB")
    width, height = image.size
    box = image.crop((width // 4, height // 4, 3 * width // 4, 3 * height // 4))
    box = box.resize((max(1, box.width // 4), max(1, box.height // 4)))
    data = box.tobytes()
    pixels = [data[i : i + 3] for i in range(0, len(data), 3)]
    blue = sum(1 for r, g, b in pixels if b >= 80 and b >= r + 40 and b >= g + 20)
    return blue / max(1, len(pixels))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--browser", required=True)
    parser.add_argument("--dist", type=Path, default=Path("dist"))
    parser.add_argument("--query", default="subdivisions=3&duration=50", help="URL parameters of the page")
    parser.add_argument("--min-updates", type=int, default=2)
    parser.add_argument("--timeout", type=float, default=180, help="seconds for the page to pass")
    parser.add_argument("--port", type=int, default=8117)
    parser.add_argument("--out", type=Path, default=Path("browser-test"))
    parser.add_argument("--apk", type=Path, help="Firefox (Fenix) APK to install on Android")
    parser.add_argument("--loaded-only", action="store_true", help="only check that the page loads (for builds before #117)")
    parser.add_argument("--hold", type=float, default=30, help="seconds the page must stay healthy after it passed")
    parser.add_argument("--min-planet", type=float, default=0.05, help="least share of ocean-blue pixels")
    args = parser.parse_args()

    dist = args.dist.resolve()
    if not (dist / "index.html").exists():
        sys.exit(f"{dist}/index.html not found: build the site first (trunk build --release)")
    args.out.mkdir(parents=True, exist_ok=True)
    token = f"test-{os.getpid()}-{int(time.time())}"
    state = State(token)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), make_handler(dist, state))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    query = f"?{args.query}" if args.query else ""
    url = f"http://localhost:{args.port}/{query}#{token}"
    print("page:", url, flush=True)

    result = {"browser": args.browser, "query": args.query, "url": url}
    screenshot = close = tick = None
    failure = None
    started = time.monotonic()
    try:
        if args.browser in ("chrome", "firefox", "safari", "edge"):
            screenshot, close, tick = open_selenium(args.browser, url)
        elif args.browser in ("webkit", "chromium-pw", "firefox-pw"):
            screenshot, close, tick = open_playwright(args.browser, url)
        elif args.browser in ("android-chrome", "android-firefox"):
            screenshot, close, tick = open_android(args.browser, url, args.port, args.apk)
        elif args.browser == "ios-safari":
            screenshot, close, tick = open_ios(url)
        else:
            sys.exit(f"unknown browser {args.browser}")
        opened = time.monotonic()
        result["open_seconds"] = round(opened - started, 1)
        last_print = last_tick = 0
        passed_at = None
        while True:
            time.sleep(0.5)
            with state.lock:
                report = json.loads(json.dumps(state.last)) if state.last else None
            if report:
                done, failure = check_state(report, args.min_updates, args.loaded_only)
                if failure:
                    break
                if done and passed_at is None:
                    passed_at = time.monotonic()
                    result["seconds"] = round(passed_at - opened, 1)
                    print(f"passed after {result['seconds']} s; watching it for {args.hold:.0f} s more", flush=True)
            if passed_at is not None and time.monotonic() - passed_at > args.hold:
                break
            if tick and passed_at is None and time.monotonic() - last_tick > 5:
                tick()
                last_tick = time.monotonic()
            if time.monotonic() - last_print > 10:
                print("waiting:", describe(report), flush=True)
                last_print = time.monotonic()
            if passed_at is None and time.monotonic() - opened > args.timeout:
                failure = f"timed out after {args.timeout:.0f} s; last state: {describe(report)}"
                break
        result.setdefault("seconds", round(time.monotonic() - opened, 1))
        # Give the view a moment to draw the newest state, then look at it.
        time.sleep(2)
        try:
            png = screenshot()
            (args.out / "screenshot.png").write_bytes(png)
            share = planet_share(png)
            result["planet_share"] = round(share, 3)
            if failure is None and share < args.min_planet:
                failure = f"the screenshot shows no planet ({share:.1%} ocean-blue pixels in the middle)"
        except Exception as error:  # noqa: BLE001 - report any screenshot failure
            result["screenshot_error"] = repr(error)
            if failure is None:
                failure = f"no screenshot: {error!r}"
    except Exception as error:  # noqa: BLE001 - a broken driver fails the test with its message
        failure = f"{type(error).__name__}: {error}"
    finally:
        with state.lock:
            state.stop = True
            result["report"] = state.last
            logs = list(state.logs)
        if close:
            try:
                close()
            except Exception as error:  # noqa: BLE001
                print("closing the browser failed:", error)
        server.shutdown()

    result["passed"] = failure is None
    result["failure"] = failure
    (args.out / "result.json").write_text(json.dumps(result, indent=2))
    (args.out / "page.log").write_text("\n".join(logs) + "\n", encoding="utf-8")
    verdict = "PASSED" if failure is None else f"FAILED: {failure}"
    print(f"{args.browser} ({args.query or 'default parameters'}): {verdict}")
    print("last state:", describe(result["report"]))
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        user_agent = (result["report"] or {}).get("userAgent", "?")
        with open(summary, "a", encoding="utf-8") as file:
            file.write(
                f"| {args.browser} | `{args.query or 'default'}` | "
                f"{'✅ passed' if failure is None else '❌ ' + failure.replace('|', '/')} | "
                f"{result.get('seconds', '?')} s | {result.get('planet_share', '?')} | {user_agent} |\n"
            )
    sys.exit(0 if failure is None else 1)


if __name__ == "__main__":
    main()
