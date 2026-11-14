#!/usr/bin/env python3
"""Capture README screenshots from the live panel (NEXUS OS) at http://localhost:8080.

Everything in the pictures is the real kernel: the script boots it from the
panel, types real shell commands into the serial console (a CPU hog, the
semaphore ping-pong and two U-mode programs) and then photographs each app.

    scripts/frontend.sh &                       # the panel must be running
    ~/.venv-shots/bin/python scripts/screenshots.py [--url URL] [--out DIR]

Needs Playwright with Chromium (pip install playwright; playwright install chromium).
"""
import argparse
import json
import time
import urllib.request
from pathlib import Path

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
APPS = ["terminal", "topo", "syscalls", "processes", "memory", "journal", "code", "files", "research", "system"]


def post(url, path, body):
    data = body if isinstance(body, bytes) else json.dumps(body).encode()
    urllib.request.urlopen(urllib.request.Request(url + path, data=data, method="POST"), timeout=10).read()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--url", default="http://localhost:8080")
    ap.add_argument("--out", default=str(ROOT / "results/screenshots"))
    ap.add_argument("--policy", default="rr")
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)

    with sync_playwright() as p:
        browser = p.chromium.launch()
        page = browser.new_page(viewport={"width": 1440, "height": 900}, device_scale_factor=1.5)
        # Fresh boot so the boot screen shows this boot's real trace.
        post(a.url, "/api/boot", {"policy": a.policy, "quantum": 4})
        page.goto(a.url, wait_until="domcontentloaded")
        page.wait_for_timeout(900)
        page.screenshot(path=str(out / "01_boot.png"))
        print("01_boot.png")
        page.wait_for_function("!document.body.classList.contains('booting')", timeout=20000)
        page.wait_for_timeout(1200)
        page.screenshot(path=str(out / "02_lock.png"))
        print("02_lock.png")
        page.keyboard.press("Enter")
        page.wait_for_timeout(1000)

        # Real work for the live views: a CPU hog, two tasks waking each
        # other through semaphores, and U-mode programs making system calls.
        # The U-mode programs run last so their output is what the terminal shows.
        # The background tasks keep running while the apps are photographed.
        # Ping-pong starts after the terminal shot so its output doesn't scroll the U-mode programs away.
        for cmd, pause in [("spin 90 &", 0.5), ("primes 40000000 &", 0.4),
                           ("clear", 0.3), ("user hello 1", 1.6), ("user priv", 1.2), ("user badptr", 1.0)]:
            post(a.url, "/api/input", (cmd + "\r").encode())
            time.sleep(pause)
        page.wait_for_timeout(1500)
        page.screenshot(path=str(out / "03_desktop.png"))
        print("03_desktop.png")

        for i, app in enumerate(APPS, start=4):
            if app == "topo":
                # Two tasks waking each other through semaphores: the Kernel Map's arcs.
                post(a.url, "/api/input", b"pingpong 900 120 &\r")
                time.sleep(2.5)
            # One app at a time on a clear desktop, so the wallpaper behind it shows too.
            page.evaluate("app => { showDesktop(); openApp(app); }", app)
            page.wait_for_timeout(1800)
            name = f"{i:02d}_{app}.png"
            page.screenshot(path=str(out / name))
            print(name)

        # Control Center over two working windows, then Mission Control.
        page.evaluate("() => { showDesktop(); openApp('terminal'); openApp('syscalls'); cc.open(); }")
        page.wait_for_timeout(1200)
        page.screenshot(path=str(out / "14_control_center.png"))
        print("14_control_center.png")
        page.evaluate("() => { cc.close(); openApp('processes'); openApp('research'); mission(true); }")
        page.wait_for_timeout(1500)
        page.screenshot(path=str(out / "15_mission_control.png"))
        print("15_mission_control.png")

        # The same desktop on a phone.
        phone = browser.new_page(viewport={"width": 375, "height": 812}, is_mobile=True, has_touch=True, device_scale_factor=2)
        phone.goto(a.url + "/#noboot", wait_until="domcontentloaded")
        phone.wait_for_timeout(4000)
        phone.screenshot(path=str(out / "16_phone.png"))
        print("16_phone.png")
        browser.close()
    print(f"saved to {out}")


if __name__ == "__main__":
    main()
