"""Read-only survey: visit each sidebar view and list its interactive controls."""
import sys, time, re
sys.path.insert(0, __file__.rsplit("/", 1)[0])
from cu import Cu
from capture import activate, ensure_geometry
cu = Cu(); activate(); ensure_geometry()
ROLES = re.compile(r"^(button|tab|pop up button|checkbox|radio button|search text field|text field|entry area|slider|toggle button|disclosure|summary|heading|combo box|link|menu button|segmented)", re.I)
views = sys.argv[1:] or ["Incoming", "Archive", "Playlist tools", "Compatible tracks", "Tracklist matcher", "Cleanup", "Library backup", "Organize files", "Sync", "Settings"]
for v in views:
    cu.key("Escape"); time.sleep(0.3)
    cu.click(rf"^button {re.escape(v)}\b"); time.sleep(1.5)
    cu.state()
    print(f"\n===== {v}")
    seen = False
    for e in cu.elements:
        if e["text"].startswith(("heading", "tab group")) or (e["depth"] >= 3 and ROLES.match(e["text"])):
            t = e["text"].replace("\n", " | ")
            if not seen and not t.startswith("heading"):  # skip sidebar chrome before the first heading
                continue
            seen = True
            if e["idx"] < 30: continue
            print(f"  {e['idx']:>3} {t[:118]}")
cu.close()
