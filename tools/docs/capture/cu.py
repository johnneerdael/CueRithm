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


def front_pid():
    """Process id of the frontmost app, or None."""
    asn = subprocess.run(["lsappinfo", "front"], capture_output=True, text=True).stdout.strip()
    out = subprocess.run(["lsappinfo", "info", "-only", "pid", asn], capture_output=True, text=True).stdout
    match = re.search(r'"pid"=(\d+)', out)
    return match.group(1) if match else None


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


LOG = os.path.expanduser("~/.cache/cuerithm-docs/open-computer-use.log")


class Cu:
    def __init__(self):
        require_capture_app()
        self.elements = []
        self.shot = None
        self._start()

    def _start(self):
        # The global pointer path (real mouse events) is needed for page controls the accessibility tree
        # does not expose, such as the table's sort headings. It is only used while the sandboxed app is
        # the frontmost app (see click_xy).
        env = dict(os.environ, OPEN_COMPUTER_USE_ALLOW_GLOBAL_POINTER_FALLBACKS="1")
        os.makedirs(os.path.dirname(LOG), exist_ok=True)
        self.proc = subprocess.Popen(["open-computer-use", "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=open(LOG, "a"), text=True, bufsize=1, env=env)
        self.n = 0
        self._send({"jsonrpc": "2.0", "id": self._id(), "method": "initialize", "params": {
            "protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "docs-capture", "version": "1"}}})
        self._read(self.n)
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

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
        try:
            self._send({"jsonrpc": "2.0", "id": ident, "method": "tools/call", "params": {"name": tool, "arguments": args}})
            reply = self._read(ident)
        except (RuntimeError, BrokenPipeError):
            # The server exits now and then. Start a fresh session; element indices from the old one are
            # void, so the caller must read the state again before it clicks by index.
            self.elements = []
            self._start()
            raise RuntimeError(f"open-computer-use restarted during {tool}; read the state again and retry") from None
        result = reply.get("result", {})
        text = "".join(c.get("text", "") for c in result.get("content", []) if c.get("type") == "text")
        image = next((c["data"] for c in result.get("content", []) if c.get("type") == "image"), None)
        if result.get("isError") or "error" in reply:
            raise RuntimeError(f"{tool} {args}: {text or reply.get('error')}")
        return text, image

    # -- state -------------------------------------------------------------
    def state(self):
        try:
            text, image = self.call("get_app_state", app=APP)
        except RuntimeError as error:
            if "restarted" not in str(error):
                raise
            text, image = self.call("get_app_state", app=APP)  # reading the state is safe to repeat
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
        hits = [e for e in self.elements if rx.search(re.sub(r"\((?:selected|focused|expanded)\) ", "", e["text"]))]
        if len(hits) <= nth:
            raise LookupError(f"no element matches /{pattern}/ (nth={nth}); {len(hits)} hits")
        return hits[nth]

    def click(self, pattern, nth=0, refresh=True, **kw):
        element = self.find(pattern, nth, refresh)
        self.call("click", app=APP, element_index=element["idx"], **kw)
        return element

    def click_xy(self, x, y, count=1):
        """Click window point (x, y) with a real mouse event. Refuses unless the sandboxed app is frontmost,
        because a global click lands on whatever window is on top at that point."""
        require_capture_app()
        with open(REGISTRY, encoding="utf-8") as handle:
            pid = handle.read().splitlines()[0]
        for _ in range(3):
            if front_pid() == pid:
                break
            # something else took the foreground (the owner works on this machine): ask for it back, then check again
            subprocess.run(["osascript", "-e", 'tell application "System Events" to set frontmost of process "CueRithm" to true'], check=False, capture_output=True)
            time.sleep(0.8)
        else:
            raise RuntimeError("the sandboxed CueRithm is not the frontmost app; not sending a pointer click")
        px, py = self.shot_point(x, y)
        self.call("click", app=APP, x=px, y=py, click_count=count, click_method="global")

    def has(self, pattern):
        rx = re.compile(pattern, re.I | re.S)
        return any(rx.search(re.sub(r"\((?:selected|focused|expanded)\) ", "", e["text"])) for e in self.elements)

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

    # press_key knows letters, digits, space and the keypad keys, but no punctuation names.
    KEYNAMES = {" ": "space", ".": "KP_Decimal", "-": "KP_Subtract", "+": "KP_Add", "/": "KP_Divide", "*": "KP_Multiply"}

    def type(self, text):
        """Type one key at a time: type_text sets the field's accessibility value, which for an empty field is
        its placeholder, and never reaches the page's input handlers."""
        for char in text:
            if char in self.KEYNAMES:
                key = self.KEYNAMES[char]
            elif char.isupper():
                key = "shift+" + char.lower()
            else:
                key = char
            self.call("press_key", app=APP, key=key)
            time.sleep(0.15)

    def close(self):
        self.proc.terminate()


def park_cursor(x=40, y=500):
    """Move the pointer off the app window so no hover highlight leaks into a capture."""
    cg = ctypes.cdll.LoadLibrary("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")

    class Point(ctypes.Structure):
        _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]

    cg.CGWarpMouseCursorPosition(Point(x, y))
