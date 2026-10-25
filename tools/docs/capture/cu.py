"""Minimal client for `open-computer-use mcp` that keeps one session open, so
element indices from get_app_state stay valid for the clicks that follow.

Element lines look like `58 cell` or `10 button Tracks 26 Help: Tracks`; rows can
continue over several lines. `Screen` parses them and clicks by label.
"""
import base64
import ctypes
import json
import os
import re
import subprocess
import time

APP = "CueRithm"
# launch-copy.sh writes the process id, start time and data copy of the sandboxed app it starts.
REGISTRY = os.path.expanduser("~/.cache/cuerithm-docs/capture-app")


def running_cuerithm():
    out = subprocess.run(["pgrep", "-x", "cuerithm-desktop"], capture_output=True, text=True).stdout
    return out.split()


def started_at(pid):
    out = subprocess.run(["ps", "-o", "lstart=", "-p", pid], capture_output=True, text=True).stdout
    return " ".join(out.split())


def require_capture_app(registry=None):
    """Stop unless the only running CueRithm is the sandboxed one launch-copy.sh started.

    Any other instance (the installed app on the owner's real library, or one started after the
    sandboxed app quit) is outside the sandbox and could rename or tag the real music files. The
    driver therefore never starts an app and checks this before it touches one.
    """
    try:
        with open(registry or REGISTRY, encoding="utf-8") as handle:
            pid, started = handle.read().splitlines()[:2]
    except (OSError, ValueError):
        raise RuntimeError("no sandboxed CueRithm is registered; start one with tools/docs/capture/launch-copy.sh") from None
    if running_cuerithm() != [pid] or started_at(pid) != started:
        raise RuntimeError("the sandboxed CueRithm is not the only CueRithm running; stop and start it again with launch-copy.sh")


class Cu:
    def __init__(self):
        require_capture_app()
        self.proc = subprocess.Popen(["open-computer-use", "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
        self.n = 0
        self._send({"jsonrpc": "2.0", "id": self._id(), "method": "initialize", "params": {
            "protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "docs-capture", "version": "1"}}})
        self._read(self.n)
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        self.elements = []
        self.shot = None

    def _id(self):
        self.n += 1
        return self.n

    def _send(self, message):
        self.proc.stdin.write(json.dumps(message) + "\n")
        self.proc.stdin.flush()

    def _read(self, want):
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("open-computer-use closed")
            try:
                message = json.loads(line)
            except ValueError:
                continue
            if message.get("id") == want:
                return message

    def call(self, tool, **args):
        require_capture_app()
        ident = self._id()
        self._send({"jsonrpc": "2.0", "id": ident, "method": "tools/call", "params": {"name": tool, "arguments": args}})
        reply = self._read(ident)
        result = reply.get("result", {})
        text = "".join(c.get("text", "") for c in result.get("content", []) if c.get("type") == "text")
        image = next((c["data"] for c in result.get("content", []) if c.get("type") == "image"), None)
        if result.get("isError") or "error" in reply:
            raise RuntimeError(f"{tool} {args}: {text or reply.get('error')}")
        return text, image

    # -- state -------------------------------------------------------------
    def state(self):
        text, image = self.call("get_app_state", app=APP)
        elements, current = [], None
        for line in text.split("\n"):
            match = re.match(r"^(\t*)(\d+) (.*)$", line)
            if match and (current is None or int(match.group(2)) > current["idx"] or match.group(1) != "" or True):
                current = {"idx": int(match.group(2)), "depth": len(match.group(1)), "text": match.group(3)}
                elements.append(current)
            elif current is not None:
                current["text"] += "\n" + line.strip()
        self.elements = elements
        self.shot = base64.b64decode(image) if image else None
        return elements

    def shot_point(self, x, y):
        """Window points (a 1440 wide window) to the pixels of the accessibility screenshot the click tool uses."""
        self.state()
        scale = int.from_bytes(self.shot[16:20], "big") / 1440
        return x * scale, y * scale

    def find(self, pattern, nth=0, refresh=True):
        if refresh:
            self.state()
        rx = re.compile(pattern, re.I | re.S)
        hits = [e for e in self.elements if rx.search(re.sub(r"\((?:selected|focused)\) ", "", e["text"]))]
        if len(hits) <= nth:
            raise LookupError(f"no element matches /{pattern}/ (nth={nth}); {len(hits)} hits")
        return hits[nth]

    def click(self, pattern, nth=0, refresh=True, **kw):
        element = self.find(pattern, nth, refresh)
        self.call("click", app=APP, element_index=element["idx"], **kw)
        return element

    def has(self, pattern):
        rx = re.compile(pattern, re.I | re.S)
        return any(rx.search(e["text"]) for e in self.elements)

    def wait_for(self, pattern, timeout=30, absent=False):
        end = time.time() + timeout
        while time.time() < end:
            self.state()
            if self.has(pattern) != absent:
                return True
            time.sleep(0.5)
        raise TimeoutError(f"/{pattern}/ {'still present' if absent else 'never appeared'}")

    def key(self, key):
        self.call("press_key", app=APP, key=key)

    def type(self, text):
        self.call("type_text", app=APP, text=text)

    def close(self):
        self.proc.terminate()


def park_cursor(x=40, y=500):
    """Move the pointer off the app window so no hover highlight leaks into a capture."""
    cg = ctypes.cdll.LoadLibrary("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")

    class Point(ctypes.Structure):
        _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]

    cg.CGWarpMouseCursorPosition(Point(x, y))
