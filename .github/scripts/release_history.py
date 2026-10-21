#!/usr/bin/env python3
"""Resolve the latest release's source across explicitly verified history migrations."""
import argparse
import json
from pathlib import Path
import re
import subprocess


def git(*args):
    return subprocess.run(
        ["git", *args], text=True, capture_output=True, check=True,
    ).stdout.strip()


def is_ancestor(source, head):
    try:
        git("merge-base", "--is-ancestor", source, head)
        return True
    except subprocess.CalledProcessError as error:
        if error.returncode == 1:
            return False
        raise


def load_migrations():
    return json.loads(Path(__file__).with_name("release_history.json").read_text())


def prior_source(head, migrations=None):
    """Return the latest global v-tag's commit or its verified equivalent baseline.

    Never substitute an older merged tag when the latest release is unrelated.
    Migration declarations are trusted tooling from main, not source-commit input.
    """
    tags = git("tag", "--list", "v*", "--sort=-v:refname").split()
    if not tags:
        return None
    latest = tags[0]
    released = git("rev-parse", "--verify", f"refs/tags/{latest}^{{commit}}")
    migrations = load_migrations() if migrations is None else migrations
    migration = migrations.get(latest)
    if migration is not None:
        for field in ("released_sha", "equivalent_main_sha"):
            if not re.fullmatch(r"[0-9a-f]{40}", migration.get(field, "")):
                raise ValueError(f"{latest}: {field} must be a full commit SHA")
        if released != migration["released_sha"]:
            raise ValueError(f"{latest}: tag target differs from declared released_sha")
        baseline = migration["equivalent_main_sha"]
        # Resolve explicitly so missing objects fail, rather than becoming an ancestry miss.
        git("rev-parse", "--verify", f"{baseline}^{{commit}}")
        if not is_ancestor(baseline, head):
            raise ValueError(f"{latest}: migration baseline {baseline} is not an ancestor of {head}")
        return baseline
    if not is_ancestor(released, head):
        raise ValueError(f"{latest}: latest release is not an ancestor of {head}; no verified migration")
    return released


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--head", default="HEAD")
    args = parser.parse_args()
    try:
        source = prior_source(args.head)
    except (ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Release history rejected: {error}\n")
    print(source or "")


if __name__ == "__main__":
    main()
