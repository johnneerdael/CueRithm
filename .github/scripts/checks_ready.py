#!/usr/bin/env python3
"""Verify every release check, optionally retaining a maintainer-declared CI baseline.

A repair commit can declare CI-Baseline and CI-Retry trailers with [skip ci].
That declaration asserts that the other components remain valid; the gate still
verifies their source-bound GitHub job results. Failed retry components can never
inherit an older success.
"""
import argparse
import json
import re
import subprocess
import sys


JOB_ROLES = {
    "engine-rust-floor": {"engine/rust-floor"},
    **{f"engine ({os})": {f"engine/{os}"} for os in
       ("ubuntu-24.04", "macos-15", "windows-2025")},
    **{f"desktop ({os})": {f"desktop/{os}"} for os in
       ("ubuntu-24.04", "macos-15", "windows-2025")},
    "windows-combined": {"engine/windows-2025", "desktop/windows-2025"},
}
REQUIRED = set().union(*JOB_ROLES.values())
WORKFLOW_ROLES = {
    "ci.yml": set().union(*(roles for name, roles in JOB_ROLES.items()
                            if name.startswith("engine") or name == "windows-combined")),
    "desktop.yml": set().union(*(roles for name, roles in JOB_ROLES.items()
                                 if name.startswith("desktop"))),
}


def retry_policy(message):
    baseline = re.findall(r"^CI-Baseline:\s*([0-9a-f]{40})\s*$", message, re.M)
    retry = re.findall(r"^CI-Retry:\s*(.+)$", message, re.M)
    if not baseline and not retry:
        return None, set()
    if len(baseline) != 1 or len(retry) != 1 or "[skip ci]" not in message.lower():
        raise ValueError("CI reuse requires one baseline, retry list and [skip ci]")
    roles = set(retry[0].split())
    if not roles or not roles <= REQUIRED:
        raise ValueError("Unknown or empty CI retry component list")
    return baseline[0], roles


def combine(current, retained=None, retry=()):
    retained = retained or {}
    combined = {}
    for role in sorted(REQUIRED):
        result = current.get(role)
        if result is None and role not in retry:
            result = retained.get(role)
        if result is not None:
            combined[role] = result
    return combined


def ready(current, retained=None, retry=()):
    combined = combine(current, retained, retry)
    missing = sorted(role for role in REQUIRED
                     if role not in combined or combined[role]["conclusion"] != "success")
    return not missing, missing


def gh(path, paginate=False):
    args = ["gh", "api", path]
    if paginate:
        args += ["--paginate", "--slurp"]
    return json.loads(subprocess.check_output(args, text=True))


def observations(repo, sha):
    found = {}
    for workflow in ("ci.yml", "desktop.yml"):
        pages = gh(f"repos/{repo}/actions/workflows/{workflow}/runs?head_sha={sha}&per_page=100", True)
        for run in (run for page in pages for run in page["workflow_runs"]):
            if run["head_sha"] != sha or run["event"] not in {"push", "workflow_dispatch"}:
                continue
            pages = gh(f"repos/{repo}/actions/runs/{run['id']}/jobs?filter=all&per_page=100", True)
            jobs = [job for page in pages for job in page["jobs"]]
            for job in jobs:
                if job["name"] not in JOB_ROLES:
                    raise ValueError(f"Unmapped release check: {job['name']}")
            attempt = run.get("run_attempt", 1)
            # Job objects can appear gradually during a rerun. Wait for the run
            # to finish before retaining results from any of its earlier attempts.
            if run.get("status", "completed") != "completed" or not any(
                    job.get("run_attempt", 1) == attempt
                    and job["conclusion"] != "skipped" for job in jobs):
                for role in WORKFLOW_ROLES[workflow]:
                    key = (run["created_at"], run["id"], attempt, 0)
                    if role not in found or key > found[role][0]:
                        found[role] = (key, {"conclusion": None, "job_id": None,
                                           "run_id": run["id"], "sha": sha})
                continue
            for job in jobs:
                if job["conclusion"] == "skipped":
                    continue
                key = (run["created_at"], run["id"], job.get("run_attempt", 1), job["id"])
                for role in JOB_ROLES[job["name"]]:
                    if role not in found or key > found[role][0]:
                        found[role] = (key, {"conclusion": job["conclusion"],
                                           "job_id": job["id"], "run_id": run["id"], "sha": sha})
    return {role: value for role, (_, value) in found.items()}


def resolve(repo, sha, chain=()):
    if sha in chain or len(chain) >= 20:
        raise ValueError("Invalid or excessive CI baseline chain")
    message = subprocess.check_output(["git", "show", "-s", "--format=%B", sha], text=True)
    baseline, retry = retry_policy(message)
    retained = {}
    if baseline:
        if baseline == sha:
            raise ValueError("CI baseline must precede the repair commit")
        subprocess.run(["git", "merge-base", "--is-ancestor", baseline, sha], check=True)
        retained = resolve(repo, baseline, (*chain, sha))
    current = observations(repo, sha)
    print(json.dumps({"sha": sha, "baseline": baseline, "retry": sorted(retry),
                      "current": current, "retained": retained}), file=sys.stderr)
    return combine(current, retained, retry)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True)
    parser.add_argument("--sha", required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.sha):
        raise ValueError("Expected a complete release commit SHA")
    ok, missing = ready(resolve(args.repo, args.sha))
    print(json.dumps({"sha": args.sha, "missing": missing}), file=sys.stderr)
    print("true" if ok else "false")


if __name__ == "__main__":
    main()
