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
APPS = ["terminal", "topo", "processes", "memory", "journal", "code", "files", "system"]


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
        page.wait_for_timeout(800)

        # Real work for the live views: a CPU hog, two tasks waking each
        # other through semaphores, and U-mode programs making system calls.
        # The U-mode programs run last so their output is what the terminal shows.
        for cmd, pause in [("spin 12 &", 0.5), ("pingpong 20 40", 3.0), ("clear", 0.3),
                           ("user hello 1", 1.6), ("user priv", 1.2), ("user badptr", 1.0)]:
            post(a.url, "/api/input", (cmd + "\r").encode())
            time.sleep(pause)
        page.wait_for_timeout(1500)

        for i, app in enumerate(APPS, start=2):
            page.evaluate("app => openApp(app)", app)
            page.wait_for_timeout(1800)
            name = f"{i:02d}_{app}.png"
            page.screenshot(path=str(out / name))
            print(name)
        browser.close()
    print(f"saved to {out}")


if __name__ == "__main__":
    main()
