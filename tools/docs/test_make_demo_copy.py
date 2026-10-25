"""Tests for the demo copy builder. Run: python3 -m unittest discover -s tools/docs"""
import json
import os
import sqlite3
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "capture"))
import make_demo_copy  # noqa: E402

SCHEMA = """
CREATE TABLE tracks(id TEXT PRIMARY KEY, title TEXT NOT NULL, path BLOB);
CREATE TABLE jobs(id INTEGER PRIMARY KEY, track_id TEXT REFERENCES tracks(id));
CREATE TABLE job_notes(id INTEGER PRIMARY KEY, job_id INTEGER REFERENCES jobs(id));
CREATE TABLE settings(revision TEXT PRIMARY KEY, json TEXT NOT NULL, created INTEGER NOT NULL);
CREATE TABLE selected_settings(singleton INTEGER PRIMARY KEY, revision TEXT NOT NULL);
CREATE TABLE meta(key TEXT PRIMARY KEY, value INTEGER);
"""
FACTS = """| # | Title | Artist |
|---|---|---|
| 1 | Atoma (Extended Mix) | Anyma (ofc) |
| 2 | Come To Life (Original Mix) | Cassian, AR/CO |
"""


class DemoCopy(unittest.TestCase):
    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.root = folder.name
        self.music = os.path.join(self.root, "music")
        os.makedirs(self.music)
        self.data = os.path.join(self.root, "data")
        os.makedirs(self.data)
        self.watch = os.path.join(self.root, "generic", "Beatport")
        db = sqlite3.connect(os.path.join(self.data, "library.sqlite"))
        db.executescript(SCHEMA)
        for ident, title, exists in [("a", "Atoma (Extended Mix)", True), ("a-old", "Atoma (Extended Mix)", False),
                                     ("c", "Come To Life (Original Mix)", True), ("x", "Another Track", True)]:
            path = os.path.join(self.music, ident + ".flac")
            if exists:
                open(path, "wb").close()
            db.execute("INSERT INTO tracks VALUES (?,?,?)", (ident, title, b"unix:" + path.encode()))
        db.execute("INSERT INTO jobs VALUES (1,'x')")
        db.execute("INSERT INTO jobs VALUES (2,'a')")
        db.execute("INSERT INTO job_notes VALUES (1,1)")
        snapshot = {"revision": "old", "settings": {"watch_folders": ["/Users/someone/Music/Beatport"]}, "models": {}}
        db.execute("INSERT INTO settings VALUES ('old',?,0)", (json.dumps(snapshot),))
        db.execute("INSERT INTO selected_settings VALUES (1,'old')")
        db.execute("INSERT INTO meta VALUES ('revision',1)")
        db.commit()
        db.close()
        self.titles = make_demo_copy.demo_titles(self.write_facts())

    def write_facts(self):
        path = os.path.join(self.root, "facts.md")
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(FACTS)
        return path

    def run_trim(self, stale):
        return make_demo_copy.trim(self.data, self.titles, stale, self.watch)

    def rows(self, sql):
        db = sqlite3.connect(os.path.join(self.data, "library.sqlite"))
        try:
            return [r[0] for r in db.execute(sql)]
        finally:
            db.close()

    def test_titles_come_from_the_facts_table(self):
        self.assertEqual(self.titles, ["Atoma (Extended Mix)", "Come To Life (Original Mix)"])

    def test_other_tracks_go_and_stale_rows_stay_when_asked(self):
        self.assertEqual(self.run_trim("keep"), (3, 1, "ok"))
        self.assertEqual(sorted(self.rows("SELECT id FROM tracks")), ["a", "a-old", "c"])

    def test_stale_rows_can_be_dropped_too(self):
        self.assertEqual(self.run_trim("drop"), (2, 2, "ok"))
        self.assertEqual(sorted(self.rows("SELECT id FROM tracks")), ["a", "c"])

    def test_rows_that_pointed_at_removed_tracks_go_with_them(self):
        self.run_trim("keep")
        self.assertEqual(self.rows("SELECT track_id FROM jobs"), ["a"])
        self.assertEqual(self.rows("SELECT count(*) FROM job_notes"), [0])

    def test_the_watch_folder_becomes_the_generic_empty_folder(self):
        self.run_trim("keep")
        selected = self.rows("SELECT s.json FROM settings s JOIN selected_settings a ON s.revision=a.revision")[0]
        self.assertEqual(json.loads(selected)["settings"]["watch_folders"], [self.watch])
        self.assertEqual(os.listdir(self.watch), [])

    def test_a_demo_title_without_a_healthy_track_stops_the_build(self):
        os.remove(os.path.join(self.music, "c.flac"))
        with self.assertRaises(SystemExit):
            self.run_trim("keep")

    def test_a_copy_with_a_switched_library_generation_is_refused_untouched(self):
        with open(os.path.join(self.data, "active-library.json"), "w", encoding="utf-8") as handle:
            handle.write("{}")
        with self.assertRaises(SystemExit):
            self.run_trim("keep")
        self.assertEqual(sorted(self.rows("SELECT id FROM tracks")), ["a", "a-old", "c", "x"])
        self.assertFalse(os.path.exists(self.watch))

    def test_a_switch_in_progress_is_refused_too(self):
        with open(os.path.join(self.data, "library-activation.json"), "w", encoding="utf-8") as handle:
            handle.write("{}")
        with self.assertRaises(SystemExit):
            self.run_trim("keep")

    def test_a_folder_without_a_library_is_refused_and_no_empty_one_is_created(self):
        os.remove(os.path.join(self.data, "library.sqlite"))
        with self.assertRaises(SystemExit):
            self.run_trim("keep")
        self.assertFalse(os.path.exists(os.path.join(self.data, "library.sqlite")))


if __name__ == "__main__":
    unittest.main()
