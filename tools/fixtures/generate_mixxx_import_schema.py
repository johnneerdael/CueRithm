#!/usr/bin/env python3
"""Generate empty test schemas from the pinned public Mixxx migration history.

Developer fixture tool only. Never accepts or migrates an existing database.
The product and Rust tests consume the generated facts/DDL without Python.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sqlite3
import xml.etree.ElementTree as ET

SCHEMA_SHA256 = "75b87df7d3b726dddd110bd8850565486f2679000f1d86684d15288a92b302c2"
TABLES = ["library", "track_locations", "cues", "Playlists", "PlaylistTracks", "crates", "crate_tracks", "directories", "settings"]


def rows(db, sql):
    cursor = db.execute(sql)
    names = [column[0] for column in cursor.description]
    return [dict(zip(names, row)) for row in cursor]


def facts(db):
    result = {"tables": {}, "indexes": {}, "foreign_keys": {}}
    for table in TABLES:
        result["tables"][table] = [
            {**{key: value for key, value in row.items() if key != "dflt_value"}, "default": row["dflt_value"]}
            for row in rows(db, f'PRAGMA table_info("{table}")')
        ]
        indexes = rows(db, f'PRAGMA index_list("{table}")')
        for index in indexes:
            index["columns"] = rows(db, f'PRAGMA index_info("{index["name"]}")')
        result["indexes"][table] = indexes
        result["foreign_keys"][table] = rows(db, f'PRAGMA foreign_key_list("{table}")')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("schema_source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    source = args.schema_source.read_bytes()
    if hashlib.sha256(source).hexdigest() != SCHEMA_SHA256:
        raise ValueError("Pinned Mixxx source SHA-256 mismatch")
    revisions = ET.fromstring(source).findall("revision")
    if len(revisions) != 40:
        raise ValueError("Unexpected migration count")
    args.output.mkdir(parents=True, exist_ok=True)
    # A new in-memory DB is the only migration target this tool can construct.
    db = sqlite3.connect(":memory:")
    profiles = {}
    manifest = json.loads((Path(__file__).resolve().parents[2] / "docs/research/dj-library-import-handover/fixtures/mixxx-schema-reference.json").read_text())
    for revision in revisions:
        version = int(revision.attrib["version"])
        sql = "".join(revision.find("sql").itertext())
        if any(token in sql.upper() for token in ["ATTACH ", "LOAD_EXTENSION", "VACUUM INTO", "TEMP_STORE_DIRECTORY"]):
            raise ValueError("Unexpected external-file operation in migration")
        db.executescript(sql)
        if version not in (39, 40):
            continue
        native = facts(db)
        if version == 40:
            for category in ("tables", "indexes", "foreign_keys"):
                for table, expected in manifest[category].items():
                    if native[category][table] != expected:
                        raise ValueError(f"Handover manifest differs: {category}.{table}")
        fingerprint = hashlib.sha256(json.dumps(native, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode()).hexdigest()
        profiles[str(version)] = {"schema_revision": version, "fingerprint": fingerprint, "facts": native}
        statements = ["-- Empty factual Mixxx schema; generated from pinned migration history.\n"]
        for table in TABLES:
            ddl = db.execute("SELECT sql FROM sqlite_master WHERE type='table' AND name=?", (table,)).fetchone()[0]
            statements.append(ddl + ";\n")
        for table in TABLES:
            for (ddl,) in db.execute("SELECT sql FROM sqlite_master WHERE type='index' AND tbl_name=? AND sql IS NOT NULL ORDER BY name", (table,)):
                statements.append(ddl + ";\n")
        for name, value in [("mixxx.schema.version", version), ("mixxx.schema.last_used_version", version), ("mixxx.schema.min_compatible_version", 3)]:
            statements.append(f"INSERT INTO settings(name,value) VALUES('{name}','{value}');\n")
        output = "\n".join(statements)
        # Independently reopen the emitted DDL before retaining a fixture.
        rebuilt = sqlite3.connect(":memory:")
        rebuilt.executescript(output)
        if facts(rebuilt) != native:
            raise ValueError(f"DDL reconstruction changed schema {version}")
        rebuilt.close()
        (args.output / f"schema{version}.sql").write_text(output)
        print(f"schema{version}: {fingerprint}")
    (args.output / "profiles.json").write_text(json.dumps(profiles, sort_keys=True, ensure_ascii=False, indent=2) + "\n")
    db.close()


if __name__ == "__main__":
    main()
