#!/usr/bin/env python3
"""Compare the schema of two Mixxx libraries object by object, showing where the text differs.

    python3 tools/mixxx-acceptance/schema_diff.py MIXXX_MADE.sqlite CUERITHM_MADE.sqlite

The acceptance run compares what each library stores in `sqlite_master` for every table, index
and view. When that differs, this shows which lines, with `repr` so that line endings and quote
characters are visible, and says whether the two spellings are the same once whitespace and
quoting are normalised.
"""
import difflib
import re
import sqlite3
import sys


def load(path):
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    try:
        return {
            (kind, name): sql
            for kind, name, sql in connection.execute(
                "SELECT type, name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'")
        }
    finally:
        connection.close()


def normalised(sql):
    """The statement without line endings, quoting or runs of whitespace."""
    return re.sub(r"\s+", " ", re.sub(r'["`\[\]]', "", sql or "")).strip().lower()


def main(first_path, second_path):
    first, second = load(first_path), load(second_path)
    differing = [key for key in sorted(set(first) | set(second)) if first.get(key) != second.get(key)]
    print(f"{len(first)} objects in {first_path}, {len(second)} in {second_path}, {len(differing)} differ")
    for key in differing:
        print(f"== {key[0]} {key[1]}")
        if key not in first or key not in second:
            print("   only in", "the second" if key not in first else "the first")
            continue
        same = normalised(first[key]) == normalised(second[key])
        print("   same once whitespace and quoting are ignored:", same)
        for line in difflib.unified_diff(
                first[key].splitlines(keepends=True), second[key].splitlines(keepends=True),
                "first", "second", n=0):
            print("   ", repr(line))
    return 1 if differing else 0


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1], sys.argv[2]))
