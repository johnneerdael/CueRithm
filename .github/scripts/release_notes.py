#!/usr/bin/env python3
"""Next release version and notes, built from merged PR `## Release notes` sections."""
import argparse
import json
import re
import subprocess

from pr_gate import sections


def next_version(base, tags):
    major, minor = base.split(".")[:2]
    patches = [int(m[1]) for t in tags if (m := re.fullmatch(rf"v{major}\.{minor}\.(\d+)", t))]
    return f"{major}.{minor}.{max(patches) + 1 if patches else 0}"


def entries(commits, bodies):
    """commits: [(sha, subject)]; bodies: {pr number: body}. Returns (public, internal, other)."""
    public, internal, other = [], [], []
    for _, subject in commits:
        pr = re.search(r"\(#(\d+)\)$", subject) or re.match(r"Merge pull request #(\d+)", subject)
        notes = sections(bodies.get(int(pr[1]), "")).get("release notes", "") if pr else ""
        lines = [re.sub(r"^[-*]\s*", "", l).strip() for l in notes.splitlines()]
        lines = [l for l in lines if l]
        if not pr:
            other.append(subject)
        elif lines and all(l.lower().startswith("internal:") for l in lines):
            internal += [l[len("internal:"):].strip() or l for l in lines]
        else:
            public += [l for l in lines if not l.lower().startswith("internal:")]
    return public, internal, other


def render(public, internal, other):
    out = ["macOS builds are signed with a Developer ID and notarized. Windows and Linux builds are not yet published.", ""]
    for title, items in [("Changes", public), ("Internal", internal), ("Direct commits", other)]:
        if items:
            out += [f"## {title}", ""] + [f"- {item}" for item in items] + [""]
    if not (public or internal or other):
        out += ["No user-facing changes.", ""]
    return "\n".join(out).rstrip() + "\n"


def run(*args):
    return subprocess.run(args, text=True, capture_output=True, check=True).stdout


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["version", "notes"])
    parser.add_argument("--base", help="Cargo workspace version")
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--repo")
    args = parser.parse_args()
    tags = run("git", "tag", "--list", "v*").split()
    if args.command == "version":
        print(next_version(args.base, tags))
        return
    ordered = run("git", "tag", "--list", "v*", "--merged", args.head, "--sort=-v:refname").split()
    prior = ordered[0] if ordered else None
    span = f"{prior}..{args.head}" if prior else args.head
    commits = [tuple(line.split("\t", 1)) for line in
               run("git", "log", "--first-parent", "--format=%H%x09%s", span).splitlines()]
    bodies = {}
    for _, subject in commits:
        pr = re.search(r"\(#(\d+)\)$", subject) or re.match(r"Merge pull request #(\d+)", subject)
        if pr:
            bodies[int(pr[1])] = json.loads(run("gh", "api", f"repos/{args.repo}/pulls/{pr[1]}")).get("body") or ""
    print(render(*entries(commits, bodies)))


if __name__ == "__main__":
    main()
