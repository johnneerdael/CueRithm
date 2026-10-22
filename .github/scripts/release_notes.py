#!/usr/bin/env python3
"""Next release version and notes, built from merged PR `## Release notes` sections."""
import argparse
import json
import re
import subprocess

from pr_gate import sections
from release_history import prior_source


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
        lines = []
        for raw in notes.splitlines():
            text = raw.strip()
            if not text:
                continue
            if re.match(r"[-*]\s", text) or not lines:
                lines.append(re.sub(r"^[-*]\s*", "", text))
            else:  # wrapped continuation of the previous bullet
                lines[-1] += " " + text
        if not pr:
            other.append(subject)
        elif lines and all(l.lower().startswith("internal:") for l in lines):
            internal += [l[len("internal:"):].strip() or l for l in lines]
        else:
            public += [l for l in lines if not l.lower().startswith("internal:")]
    return public, internal, other


PRODUCT = "CueRithm"
TAGLINE = "Know your music. Shape your set."
RELEASE_BANNER = "docs/assets/brand/release-banner.png"
# Use the supplied dark 1600x480 brand-kit banner unchanged.
# One row per platform package that release.yml publishes; keep the two in step.
DOWNLOADS = [
    ("macOS DMG", "Apple Silicon (M1 and later); there is no Intel build", "Developer ID signed and Apple notarized", "Cuerithm-arm64-{version}.dmg"),
    ("Windows portable ZIP", "x64", "unsigned; Windows SmartScreen may warn on first launch", "Cuerithm-windows-x64-{version}.zip"),
    ("Linux DEB", "x64", "unsigned package", "Cuerithm-linux-x64-{version}.deb"),
    ("Application source", "All platforms", "CueRithm source; bundled FFmpeg source is in the assets below", "cuerithm-source.tar.gz"),
]


def banner(repo, sha):
    """Unmodified release banner, pinned to the source before the release tag exists."""
    url = f"https://raw.githubusercontent.com/{repo}/{sha}/{RELEASE_BANNER}"
    return [f"![{PRODUCT}. {TAGLINE}]({url})", ""]


def status(repo, sha, version):
    stage = " (pre-1.0)" if version and version.startswith("0.") else ""
    return f"**Release{stage}.** Source: [{sha[:12]}](https://github.com/{repo}/commit/{sha})."


def downloads(repo=None, version=None):
    rows = ["| Download | Platform | Signing / distribution |", "| --- | --- | --- |"]
    base = f"https://github.com/{repo}/releases/download/v{version}" if repo and version else None
    for name, platform, signing, filename in DOWNLOADS:
        label = f"[{name}]({base}/{filename.format(version=version)})" if base else name
        rows.append(f"| {label} | {platform} | {signing} |")
    if base:
        rows += ["", f"[Third-party license notices]({base}/THIRD_PARTY_NOTICES.md)."]
    return ["## Downloads", ""] + rows + [""]


def render(public, internal, other, repo=None, sha=None, version=None):
    branded = bool(repo and sha)
    out = banner(repo, sha) + [f"{PRODUCT} — {TAGLINE}", ""] if branded else []
    if branded and version and version.startswith("0."):
        out += ["**Pre-1.0 release.**", ""]
    # People using the app read the top; developer detail stays available but collapsed.
    if public:
        out += ["## What's new", ""] + [f"- {item}" for item in public] + [""]
    else:
        out += ["No user-facing changes.", ""]
    if branded:
        out += downloads(repo, version)
    developer = [(title, items) for title, items in [("Internal", internal), ("Direct commits", other)] if items]
    if branded or developer:
        out += ["<details>", "<summary>Build and source verification</summary>", ""]
        if branded:
            out += [status(repo, sha, version), ""]
        for title, items in developer:
            out += [f"**{title}**", ""] + [f"- {item}" for item in items] + [""]
        out += ["</details>"]
    return "\n".join(out).rstrip() + "\n"


def run(*args):
    return subprocess.run(args, text=True, capture_output=True, check=True).stdout


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["version", "notes", "has-public"])
    parser.add_argument("--base", help="Cargo workspace version")
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--repo")
    parser.add_argument("--version", help="Release version, used for the stage label on the page")
    args = parser.parse_args()
    tags = run("git", "tag", "--list", "v*").split()
    if args.command == "version":
        print(next_version(args.base, tags))
        return
    prior = prior_source(args.head)
    span = f"{prior}..{args.head}" if prior else args.head
    commits = [tuple(line.split("\t", 1)) for line in
               run("git", "log", "--first-parent", "--format=%H%x09%s", span).splitlines()]
    bodies = {}
    for _, subject in commits:
        pr = re.search(r"\(#(\d+)\)$", subject) or re.match(r"Merge pull request #(\d+)", subject)
        if pr:
            bodies[int(pr[1])] = json.loads(run("gh", "api", f"repos/{args.repo}/pulls/{pr[1]}")).get("body") or ""
    result = entries(commits, bodies)
    if args.command == "has-public":
        print("true" if result[0] else "false")
        return
    print(render(*result, repo=args.repo, sha=run("git", "rev-parse", args.head).strip(), version=args.version))


if __name__ == "__main__":
    main()
