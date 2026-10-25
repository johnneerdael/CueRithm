def player_times(elements):
    """(elapsed, remaining) seconds from the player readout. The remaining time is shown after a minus, either inside one
    element ("−4:13.4") or as its own element followed by the number; the waveform's 0:00.0 and 0:32.0 labels also match
    the clock format, so they are never taken for either."""
    elapsed = remaining = None
    after_minus = False
    for element in elements:
        text = element["text"]
        if re.fullmatch(r"text [\u2212-]", text):
            after_minus = True
            continue
        match = re.fullmatch(r"text ([\u2212-]?)(\d+:\d\d\.\d)", text)
        if not match:
            after_minus = False
            continue
        if match.group(1) or after_minus:
            remaining = remaining if remaining is not None else clock(match.group(2))
        elif elapsed is None:
            elapsed = clock(match.group(2))
        after_minus = False
    if elapsed is None or remaining is None or elapsed + remaining <= 0:
        raise LookupError("the player's elapsed and remaining time are not on screen")
    return elapsed, remaining


def seek_to(cu, seconds):
    """Click the track overview where `seconds` falls; a pointer click is the only thing it listens to."""
    cu.state()
    elapsed, remaining = player_times(cu.elements)
    box = json.loads(cu.call("query", app="CueRithm", text="Track overview")[0])["matches"][0]["bounds"]
    x = box["x"] + box["w"] * seconds / (elapsed + remaining)
    cu.click_xy(x, box["y"] + box["h"] / 2)
    time.sleep(0.8)


#!/usr/bin/env python3
"""Capture user-guide screenshots from the running CueRithm window.

Usage:
  capture.py [--themes dark,light] [--out DIR] [--only a,b,c] [--file scenes/x.json ...]
  capture.py --shot NAME [--themes dark]        # capture the current screen as-is

A scene file is a JSON array of {"scene": "name", "steps": [[verb, arg...], ...]}.
Verbs: click PATTERN [NTH] | dblclick PATTERN [NTH] | key KEY [COUNT] | type TEXT | set PATTERN VALUE |
       scroll PATTERN DIR PAGES | clickxy X Y | dblclickxy X Y | drag X1 Y1 X2 Y2 (window points) |
       wait SECONDS | wait_for PATTERN [SECONDS] | gone PATTERN | reset | view NAME |
       sort HEADING [CLICKS] | play TITLE | seekto SECONDS |
       columns | tick NAME on|off | place NAME OTHER | fill LABEL TEXT | scrollto TEXT [TOP] | search TEXT | colsearch TEXT | blur | row N [cmd|shift] | deselect | clicktext TEXT [cmd|shift|plain] [NTH]
A scene may carry "after": [steps] that run once the picture is taken, to undo what it changed, and "crop": "window" (default) | "panel" | "player"; without it the crop requested in
docs/user-guide/_meta/figures/*.json for that scene id is used.
Patterns are case-insensitive regular expressions over `<role> <label>` of an
accessibility element. Raw captures are PNG with the real window shape; `to_webp.py`
downsizes them. Needs the sandboxed CueRithm that launch-copy.sh started (the only one running) and its window at 1440x940.
"""
import argparse
import glob
import json
import os
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cu import Cu, park_cursor, require_capture_app  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
FIGURES = os.path.join(HERE, "..", "..", "..", "docs", "user-guide", "_meta", "figures")
# Window points at the 1440x940 size: the main panel is right of the sidebar and above the player.
CROPS = {"window": None, "panel": (336, 0, 1440, 737), "player": (336, 737, 1440, 940)}


# Window points of the table headings at 1440x940 with the default columns and no row selected.
HEADINGS = {"TRACK": 468, "ARTIST": 530, "GENRE": 780, "BPM": 913, "KEY": 996, "ENERGY": 1070, "DURATION": 1181}
HEADING_Y = 241
VOLUME = "0.65"


def requested_crops():
    crops = {}
    for path in glob.glob(os.path.join(FIGURES, "*.json")):
        with open(path, encoding="utf-8") as handle:
            for figure in json.load(handle):
                crops.setdefault(figure["scene"], figure.get("crop", "window"))
    return crops


def windowid_tool():
    """Path of the compiled window-id helper; built from windowid.swift on first use."""
    path = os.environ.get("CUERITHM_WINDOWID") or os.path.expanduser("~/.cache/cuerithm-docs/windowid")
    if not os.path.exists(path):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        subprocess.run(["swiftc", "-O", os.path.join(HERE, "windowid.swift"), "-o", path], check=True)
    return path


def window():
    out = subprocess.run([windowid_tool(), "CueRithm"], capture_output=True, text=True).stdout.split()
    if len(out) != 5:
        raise RuntimeError("CueRithm window not found on screen")
    return [int(v) for v in out]


def ensure_geometry():
    wid, x, y, w, h = window()
    if (w, h) != (1440, 940):
        subprocess.run(["osascript", "-e", 'tell application "System Events" to tell process "CueRithm" to set size of window 1 to {1440, 940}'], check=False)
        time.sleep(0.8)


def activate():
    """Bring the running sandboxed CueRithm to the front. It never starts an app: `open -a` would
    start the installed one on the real library, outside the sandbox, if the sandboxed one had quit."""
    require_capture_app()
    subprocess.run(["osascript", "-e", 'tell application "System Events" to set frontmost of process "CueRithm" to true'], check=True)
    time.sleep(0.6)


def shoot(name, theme, out, crop="window"):
    activate()
    park_cursor()
    time.sleep(0.5)
    wid = window()[0]
    path = os.path.join(out, f"{name}-{theme}.png")
    subprocess.run(["screencapture", "-x", "-o", f"-l{wid}", path], check=True)
    if CROPS[crop]:
        from PIL import Image
        image = Image.open(path)
        scale = image.width / 1440
        image.crop(tuple(round(v * scale) for v in CROPS[crop])).save(path)
    return path


def current_theme(cu):
    cu.state()
    for e in cu.elements:
        if e["text"].startswith("button Colour theme"):
            return e["text"].split("Colour theme: ")[1].split(" ")[0]
    return None


def set_theme(cu, target):
    for _ in range(4):
        have = current_theme(cu)
        if have == target:
            return
        cu.click(r"^button Colour theme", refresh=False)
        time.sleep(0.8)
    raise RuntimeError(f"could not reach theme {target}")


def sorted_by(cu):
    cu.state()
    for i, e in enumerate(cu.elements):
        if e["text"].startswith("text SORTED") and i + 1 < len(cu.elements):
            return cu.elements[i + 1]["text"].replace("text ", "")
    return None


def title_order(cu):
    """True while the table is sorted by title, A to Z: the demo library's second row is Atoma."""
    cu.state()
    return sorted_by(cu) == "TITLE" and any(e["text"].startswith("row 2") and "Atoma" in e["text"] for e in cu.elements)


def reset(cu):
    """Back to the plain Tracks table: nothing selected, search empty, sorted by title, player paused, silent."""
    activate()
    cu.key("Escape")
    time.sleep(0.2)
    cu.key("Escape")
    time.sleep(0.2)
    cu.state()
    for panel in (r"^button Close columns", r"^button Close corrections"):
        if cu.has(panel):
            cu.click(panel, refresh=False)
            time.sleep(0.6)
            cu.state()
    cu.click(r"^button Archive")  # leaving the view clears the selection
    time.sleep(0.5)
    cu.click(r"^button Tracks \d")
    time.sleep(0.8)
    cu.state()
    field = cu.find(r"^search text field", refresh=False)
    if "Value:" in field["text"]:  # an empty field only lists its placeholder
        search(cu, "")
    cu.state()
    if cu.has(r"^toggle button Waveform zoom: Entire track"):  # the zoom is remembered between tracks
        cu.click(r"^toggle button Waveform zoom", refresh=False)
        time.sleep(0.5)
    for _ in range(3):
        if title_order(cu):
            break
        cu.click_xy(HEADINGS["TRACK"], HEADING_Y)
        time.sleep(0.8)
    cu.click_xy(150, 62)  # a pointer click on the brand text clears the keyboard focus ring
    time.sleep(0.4)
    volume(cu, "0")


def volume(cu, value):
    cu.state()
    cu.call("set_value", app="CueRithm", element_index=cu.find(r"^slider .*Volume", refresh=False)["idx"], value=value)


def pause_playback(cu):
    cu.state()
    if cu.has(r"^button Pause"):
        cu.click(r"^button Pause", refresh=False)
        time.sleep(0.5)


ROW_CENTRE = {False: 286.5, True: 329.5}  # first table row, window points, without / with the selection toolbar
ROW_HEIGHT = 56


def press_with(modifier, x, y, tap=0):
    """A real left click at window point (x, y) with Command or Shift held; the click tool cannot hold a modifier.
    Posted to the whole screen, so it only runs while the sandboxed app is frontmost."""
    from cu import front_pid, REGISTRY
    require_capture_app()
    with open(REGISTRY, encoding="utf-8") as handle:
        pid = handle.read().splitlines()[0]
    if front_pid() != pid:
        raise RuntimeError("the sandboxed CueRithm is not the frontmost app; not sending a pointer click")
    import ctypes
    _, left, top, _, _ = window()
    cg = ctypes.cdll.LoadLibrary("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")

    class Point(ctypes.Structure):
        _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]

    cg.CGEventCreateMouseEvent.restype = ctypes.c_void_p
    cg.CGEventCreateMouseEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint32, Point, ctypes.c_uint32]
    cg.CGEventSetFlags.argtypes = [ctypes.c_void_p, ctypes.c_uint64]
    cg.CGEventPost.argtypes = [ctypes.c_uint32, ctypes.c_void_p]
    cg.CFRelease.argtypes = [ctypes.c_void_p]
    flag = {"none": 0, "cmd": 0x100000, "shift": 0x20000}[modifier]
    point = Point(left + x, top + y)
    cg.CGWarpMouseCursorPosition(point)
    time.sleep(0.15)
    for kind in (1, 2):  # left mouse down, up
        event = cg.CGEventCreateMouseEvent(None, kind, point, 0)
        cg.CGEventSetFlags(event, flag)
        cg.CGEventPost(tap, event)
        cg.CFRelease(event)
        time.sleep(0.08)
    time.sleep(0.6)


def ocr_tool():
    path = os.environ.get("CUERITHM_OCRBOXES") or os.path.expanduser("~/.cache/cuerithm-docs/ocrboxes")
    if not os.path.exists(path):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        subprocess.run(["swiftc", "-O", os.path.join(HERE, "ocrboxes.swift"), "-o", path], check=True)
    return path


def find_text(text, nth=0):
    """Window point at the centre of the nth line of visible text containing `text`: for controls and rows the
    accessibility tree does not list."""
    import tempfile
    wid, _, _, _, _ = window()
    with tempfile.TemporaryDirectory() as folder:
        path = os.path.join(folder, "window.png")
        subprocess.run(["screencapture", "-x", "-o", f"-l{wid}", path], check=True)
        from PIL import Image
        scale = Image.open(path).width / 1440
        lines = subprocess.run([ocr_tool(), path], capture_output=True, text=True, check=True).stdout.splitlines()
    hits = []
    for line in lines:
        x, y, w, h, found = line.split("\t", 4)
        if text.lower() in found.lower():
            hits.append(((int(x) + int(w) / 2) / scale, (int(y) + int(h) / 2) / scale))
    if len(hits) <= nth:
        raise LookupError(f"no text /{text}/ on screen (nth={nth}); {len(hits)} hits")
    return hits[nth]


def click_row(cu, number, how="plain"):
    """Click the Nth visible table row (1 is the top one); `cmd` adds it to the selection, `shift` extends it."""
    cu.state()
    selected = cu.has(r"^text \d+ selected")
    y = ROW_CENTRE[selected] + ROW_HEIGHT * (number - 1)
    if how == "plain":
        cu.click_xy(700, y)
    else:
        press_with(how, 700, y)


def scroll_page(cu, direction, pages):
    """Scroll the page body. The scroll tool acts where its element is, so use a control that is on screen now
    (a query lists only visible elements); controls scrolled out of view leave nothing to scroll."""
    for role in ("checkbox", "radio button", "text field", "number field", "pop up button", "button"):
        found = json.loads(cu.call("query", app="CueRithm", role=role)[0])["matches"]
        found = [m for m in found if m["bounds"]["x"] > 360 and 230 < m["bounds"]["y"] < 700]
        if found:
            cu.call("scroll", app="CueRithm", element_index=str(found[0]["index"]), direction=direction, pages=pages)
            time.sleep(0.7)
            return
    raise RuntimeError("nothing on screen to scroll with")


def search(cu, text):
    """Type into the search box the way a person does: set_value would not reach the page's input handler."""
    cu.click_xy(450, 32)
    time.sleep(0.4)
    cu.key("super+a")
    if text:
        cu.type(text)
    else:
        cu.key("BackSpace")
    time.sleep(1.2)


def clock(text):
    """Seconds in a player readout such as 4:20.6."""
    minutes, seconds = text.split(":")
    return int(minutes) * 60 + float(seconds)


def seek_to(cu, seconds):
    """Click the track overview where `seconds` falls; a pointer click is the only thing it listens to."""
    cu.state()
    times = [e["text"].replace("text ", "") for e in cu.elements if re.fullmatch(r"text \d+:\d\d\.\d", e["text"])]
    elapsed, remaining = clock(times[0]), clock(times[1])
    box = json.loads(cu.call("query", app="CueRithm", text="Track overview")[0])["matches"][0]["bounds"]
    x = box["x"] + box["w"] * seconds / (elapsed + remaining)
    cu.click_xy(x, box["y"] + box["h"] / 2)
    time.sleep(0.8)


def attempt(action):
    """Run a step; if the shared computer-use agent dropped the session, run it once more (every step reads the state first)."""
    try:
        return action()
    except RuntimeError as error:
        if "restarted" not in str(error):
            raise
        time.sleep(0.5)
        return action()


def run_steps(cu, steps):
    for step in steps:
        attempt(lambda step=step: run_step(cu, step))


def run_step(cu, step):
    verb, args = step[0], step[1:]
    if verb == "click":
        cu.click(args[0], int(args[1]) if len(args) > 1 else 0)
        time.sleep(0.7)
    elif verb == "dblclick":
        cu.click(args[0], int(args[1]) if len(args) > 1 else 0, click_count=2)
        time.sleep(1.2)
    elif verb in ("clickxy", "dblclickxy"):
        cu.click_xy(float(args[0]), float(args[1]), 2 if verb == "dblclickxy" else 1)
        time.sleep(1.2 if verb == "dblclickxy" else 0.7)
    elif verb == "drag":
        (x1, y1), (x2, y2) = cu.shot_point(float(args[0]), float(args[1])), cu.shot_point(float(args[2]), float(args[3]))
        cu.call("drag", app="CueRithm", from_x=x1, from_y=y1, to_x=x2, to_y=y2)
        time.sleep(0.8)
    elif verb == "key":
        for _ in range(int(args[1]) if len(args) > 1 else 1):
            cu.key(args[0])
            time.sleep(0.25)
        time.sleep(0.4)
    elif verb == "columns":
        cu.state()
        if cu.has(r"^button Close columns"):
            return  # already open: the button would close it
        cu.call("scroll", app="CueRithm", element_index=cu.find(r"^table")["idx"], direction="right", pages=3)  # the button sits at the end of a wide header
        time.sleep(0.6)
        cu.click_xy(1400, 242)
        time.sleep(1.0)
    elif verb == "tick":
        # tick NAME on|off: only clicks when the checkbox is not already in that state
        box = cu.find(rf"^checkbox {re.escape(args[0])} Value: (on|off)")
        if (" Value: on" in box["text"]) != (args[1] == "on"):
            cu.click(rf"^checkbox {re.escape(args[0])} Value", refresh=False)
        time.sleep(0.5)
    elif verb == "place":
        # place NAME OTHER: move the ticked column NAME until it sits right after OTHER in the panel's order
        for _ in range(14):
            cu.state()
            order = [re.match(r"checkbox (.*) Value: on", e["text"]).group(1) for e in cu.elements if re.match(r"checkbox .* Value: on", e["text"])]
            gap = order.index(args[0]) - order.index(args[1])
            if gap == 1:
                break
            cu.click(rf"^button Move {re.escape(args[0])} {'left' if gap > 1 else 'right'}", refresh=False)
            time.sleep(0.4)
    elif verb == "fill":
        # fill LABEL TEXT: a number or text box by its accessibility label, typed with real key presses
        matches = json.loads(cu.call("query", app="CueRithm", text=args[0])[0])["matches"]
        box = next(m for m in matches if m.get("title") == args[0])["bounds"]
        if "." in args[1]:
            # Keypad "." types the system's decimal comma and a controlled number box drops "59." while typing:
            # set the value, then step it up and down so the page reads it.
            cu.click_xy(box["x"] + box["w"] / 2, box["y"] + box["h"] / 2)
            time.sleep(0.4)
            cu.call("set_value", app="CueRithm", element_index=cu.find(rf"^number field .*{re.escape(args[0])}")["idx"], value=args[1])
            time.sleep(0.3)
            cu.key("Up")
            time.sleep(0.3)
            cu.key("Down")
        else:
            cu.click_xy(box["x"] + box["w"] / 2, box["y"] + box["h"] / 2)
            time.sleep(0.3)
            cu.key("super+a")
            cu.type(args[1])
        time.sleep(0.6)
    elif verb == "scrollto":
        # scrollto TEXT [TOP]: scroll the page until the first visible line of text containing TEXT sits TOP points
        # (default 190) below the window top. Reads the screen, because off-screen elements have no bounds.
        want = float(args[1]) if len(args) > 1 else 190.0
        for _ in range(30):
            try:
                y = find_text(args[0])[1]
            except LookupError:
                y = None
            if y is not None and abs(y - want) <= 35:
                break
            scroll_page(cu, "up" if (y is not None and y < want) else "down", 0.5 if (y is None or abs(y - want) > 300) else 0.12)
    elif verb == "blur":
        cu.click_xy(150, 62)  # a pointer click on the brand text clears a keyboard focus ring
        time.sleep(0.4)
    elif verb == "colsearch":
        cu.click_xy(1245, 321)  # the Table columns panel's Find box; a real click gives it keyboard focus
        time.sleep(0.3)
        cu.key("super+a")
        if args[0]:
            cu.type(args[0])
        else:
            cu.key("BackSpace")
        time.sleep(1.2)
    elif verb == "search":
        search(cu, args[0])
    elif verb == "type":
        cu.type(args[0])
        time.sleep(0.5)
    elif verb == "sort":
        for _ in range(int(args[1]) if len(args) > 1 else 1):
            cu.click_xy(HEADINGS[args[0].upper()], HEADING_Y)
            time.sleep(0.7)
    elif verb == "play":
        cu.wait_for(rf"^row.*{re.escape(args[0])}", timeout=15)
        cu.click(rf"^row.*{re.escape(args[0])}", refresh=False)  # the play button appears inside the selected row
        time.sleep(0.6)
        cu.click(rf"^button Play .*{re.escape(args[0])}")
        cu.wait_for(r"^button Pause", timeout=20)
        cu.click(r"^button Pause", refresh=False)  # Space would press whichever control has keyboard focus
        cu.wait_for(r"^button Play$", timeout=10)
    elif verb == "clicktext":
        x, y = find_text(args[0], int(args[2]) if len(args) > 2 else 0)
        how = args[1] if len(args) > 1 else "plain"
        if how == "plain":
            cu.click_xy(x, y)
        else:
            press_with(how, x, y)
        time.sleep(0.7)
    elif verb == "row":
        click_row(cu, int(args[0]), args[1] if len(args) > 1 else "plain")
    elif verb == "deselect":
        cu.click(r"^button Archive")  # leaving the view clears the selection; the player keeps its track
        time.sleep(0.5)
        cu.click(r"^button Tracks \d")
        time.sleep(0.8)
        cu.click_xy(150, 62)
        time.sleep(0.4)
    elif verb == "seekto":
        seek_to(cu, float(args[0]))
    elif verb == "set":
        e = cu.find(args[0])
        cu.call("set_value", app="CueRithm", element_index=e["idx"], value=args[1])
        time.sleep(0.8)
    elif verb == "scroll":
        e = cu.find(args[0])
        cu.call("scroll", app="CueRithm", element_index=e["idx"], direction=args[1], pages=float(args[2]))
        time.sleep(0.6)
    elif verb == "wait":
        time.sleep(float(args[0]))
    elif verb == "wait_for":
        cu.wait_for(args[0], timeout=float(args[1]) if len(args) > 1 else 30)
    elif verb == "gone":
        cu.wait_for(args[0], timeout=60, absent=True)
    elif verb == "reset":
        reset(cu)
    elif verb == "view":
        cu.click(rf"^button {args[0]}\b")
        time.sleep(0.8)
    else:
        raise ValueError(f"unknown verb {verb}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--themes", default="dark,light")
    parser.add_argument("--out", default="/tmp/cuerithm-raw")
    parser.add_argument("--only", default="")
    parser.add_argument("--file", nargs="*")
    parser.add_argument("--shot")
    args = parser.parse_args()
    os.makedirs(args.out, exist_ok=True)
    cu = Cu()
    activate()
    ensure_geometry()
    themes = args.themes.split(",")
    if args.shot:
        for theme in themes:
            set_theme(cu, theme)
            print(shoot(args.shot, theme, args.out))
        return 0
    files = args.file or sorted(glob.glob(os.path.join(HERE, "scenes", "*.json")))
    scenes = []
    for path in files:
        with open(path, encoding="utf-8") as handle:
            scenes += json.load(handle)
    wanted = set(filter(None, args.only.split(",")))
    crops = requested_crops()
    failures = []
    attempt(lambda: volume(cu, "0"))  # never play audio out loud
    current = current_theme(cu)
    for scene in scenes:
        if wanted and scene["scene"] not in wanted:
            continue
        crop = scene.get("crop") or crops.get(scene["scene"], "window")
        # Switching the theme with the scene still on screen saves the steps, but it dismisses a menu or popover,
        # so those scenes ("retheme": false) run again from the start for each theme. The theme the app is in
        # goes first, so consecutive scenes alternate instead of switching twice.
        order = sorted(themes, key=lambda t: t != current)
        retheme = scene.get("retheme", True)
        done = False
        for theme in order:
            try:
                if done and retheme:
                    attempt(lambda: set_theme(cu, theme))
                    attempt(lambda: run_step(cu, ["blur"]))
                else:
                    attempt(lambda: set_theme(cu, theme))
                    attempt(lambda: reset(cu))
                    run_steps(cu, scene["steps"])
                    attempt(lambda: pause_playback(cu))
                    attempt(lambda: volume(cu, VOLUME))  # silent while playing, but the player shows its normal volume
                    done = True
                time.sleep(float(scene.get("settle", 0.8)))
                print("ok ", theme, shoot(scene["scene"], theme, args.out, crop))
            except Exception as error:  # keep going: report at the end
                failures.append((theme, scene["scene"], str(error)[:200]))
                print("FAIL", theme, scene["scene"], str(error)[:200])
            current = theme
            if not retheme or theme == order[-1]:
                try:
                    run_steps(cu, scene.get("after", []))  # put back what the scene changed (columns, drafts)
                except Exception as error:
                    print("WARN steps after", scene["scene"], "failed:", str(error)[:150])
                done = False if not retheme else done
    try:
        volume(cu, VOLUME)
    except Exception:
        pass
    cu.close()
    for failure in failures:
        print("FAILED", *failure)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
