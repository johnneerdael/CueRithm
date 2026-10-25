#!/usr/bin/env python3
"""Turn a COPY of the owner's CueRithm data into the guide's demo library.

The guide's text quotes one small library: the 26 FLAC files listed in the private facts sheet
(`.analysis/guide/library-facts.md`, not in Git). The owner's library has grown since, so a capture
from it would contradict the guide's own numbers ("Energy 7 and up: 9 tracks"). This keeps those
26 tracks, can keep the stale rows left by files that moved (the broken-library scenes), removes
everything else from the copy, and points the watch folder at a generic empty folder so no screen
shows a user name.

Usage: make_demo_copy.py DATA_COPY FACTS_SHEET [--stale keep|drop]
Run it on a copy with the sandboxed app stopped; it refuses the live data folder. It never opens a
music file for writing: it checks that files exist and edits only DATA_COPY/library.sqlite.
"""
import argparse
import hashlib
import json
import os
import re
import sqlite3
import sys
import time

GENERIC_WATCH = "/private/tmp/demo/Music/Beatport"
LIVE = os.path.realpath(os.path.expanduser("~/Library/Application Support/com.cuerithm.app"))


def demo_titles(facts):
    titles = []
    with open(facts, encoding="utf-8") as handle:
        for line in handle:
            cells = [c.strip() for c in line.split("|")]
            if len(cells) > 3 and re.fullmatch(r"\d+", cells[1]):
                titles.append(cells[2])
    return titles


def real_path(blob):
    text = blob.decode() if isinstance(blob, bytes) else blob
    return text[5:] if text.startswith("unix:") else text


def trim(data, titles, stale="keep", watch=GENERIC_WATCH):
    """Keep one healthy track per demo title (and the stale rows when asked); returns (kept, removed, integrity)."""
    db = sqlite3.connect(os.path.join(data, "library.sqlite"))
    db.execute("PRAGMA foreign_keys=OFF")
    rows = db.execute("SELECT id,title,path FROM tracks").fetchall()
    healthy, stale_ids = {}, []
    for ident, title, path in rows:
        if title not in titles:
            continue
        if os.path.exists(real_path(path)):
            healthy.setdefault(title, []).append(ident)
        else:
            stale_ids.append(ident)
    missing = [t for t in titles if len(healthy.get(t, [])) != 1]
    if missing:
        raise SystemExit(f"each demo title must have exactly one healthy track; check: {missing}")
    keep = {ids[0] for ids in healthy.values()} | (set(stale_ids) if stale == "keep" else set())
    gone = [(ident,) for ident, _, _ in rows if ident not in keep]

    db.execute("CREATE TEMP TABLE gone(id TEXT PRIMARY KEY)")
    db.executemany("INSERT INTO gone VALUES (?)", gone)
    for table in [r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table'")]:
        for fk in db.execute(f'PRAGMA foreign_key_list("{table}")').fetchall():
            if fk[2] == "tracks":
                db.execute(f'DELETE FROM "{table}" WHERE "{fk[3]}" IN (SELECT id FROM gone)')
    db.execute("DELETE FROM tracks WHERE id IN (SELECT id FROM gone)")
    for _ in range(8):  # rows that pointed at the rows just removed
        broken = db.execute("PRAGMA foreign_key_check").fetchall()
        if not broken:
            break
        for table, rowid, _, _ in broken:
            if rowid is None:
                raise SystemExit(f"cannot clean {table}: a table without rowid points at a removed row")
            db.execute(f'DELETE FROM "{table}" WHERE rowid=?', (rowid,))
    else:
        raise SystemExit("foreign key violations remain")

    # A generic, empty watch folder: the settings screens show it, and nothing in it can be picked up.
    os.makedirs(watch, exist_ok=True)
    raw = db.execute("SELECT s.json FROM settings s JOIN selected_settings a ON s.revision=a.revision").fetchone()[0]
    snapshot = json.loads(raw)
    snapshot["settings"]["watch_folders"] = [watch]
    revision = hashlib.sha256(json.dumps({"settings": snapshot["settings"], "models": snapshot["models"]}, sort_keys=True).encode()).hexdigest()
    snapshot["revision"] = revision
    db.execute("INSERT OR IGNORE INTO settings VALUES (?,?,?)", (revision, json.dumps(snapshot, separators=(",", ":")), int(time.time() * 1000)))
    db.execute("UPDATE selected_settings SET revision=? WHERE singleton=1", (revision,))
    db.execute("UPDATE meta SET value=value+1 WHERE key='revision'")
    db.commit()
    db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
    integrity = db.execute("PRAGMA integrity_check").fetchone()[0]
    kept = db.execute("SELECT count(*) FROM tracks").fetchone()[0]
    db.close()
    return kept, len(gone), integrity


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("data")
    parser.add_argument("facts")
    parser.add_argument("--stale", choices=("keep", "drop"), default="keep")
    args = parser.parse_args()
    data = os.path.realpath(args.data)
    if data == LIVE or data.startswith(LIVE + os.sep):
        sys.exit("refusing the live data folder")
    titles = demo_titles(args.facts)
    if len(titles) != 26 or len(set(titles)) != 26:
        sys.exit(f"expected 26 distinct titles in the facts sheet, found {len(titles)}")
    kept, removed, integrity = trim(data, titles, args.stale)
    print(f"kept {kept} tracks, removed {removed}; integrity {integrity}; watch folder {GENERIC_WATCH}")
    return 0 if integrity == "ok" else 1


if __name__ == "__main__":
    sys.exit(main())
