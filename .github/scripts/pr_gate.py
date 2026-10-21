#!/usr/bin/env python3
"""Lean PR gate: PR body sections plus a completed Codex review of the head commit.

Once the gate passes it starts the Linux checks for that commit (one dispatch per commit), and a
newer commit cancels unfinished runs of the old one. Pushes alone never start them. Every other
platform runs after the merge, on main, and a failure there is fixed on main.

Run only from main with a token that never executes PR code.
"""
import argparse
import json
import os
import re
import subprocess
import urllib.parse

CONTEXT = "PR gate"
CODEX = "chatgpt-codex-connector[bot]"
HOSTED_CHECKS = ("ci.yml", "desktop.yml")
PR_RUNNER = "ubuntu-24.04"  # before the merge only Linux runs; Windows and macOS run on main


def sections(body):
    body = re.sub(r"<!--.*?-->", "", body or "", flags=re.S)
    parts = re.split(r"^##[ \t]+(.+?)[ \t]*$", body, flags=re.M)
    return {parts[i].strip().lower(): parts[i + 1].strip() for i in range(1, len(parts) - 1, 2)}


def validate_body(body):
    found = sections(body)
    notes = re.sub(r"^[-*]\s*", "", found.get("release notes", ""), flags=re.M).strip()
    if not notes or notes.lower() == "internal:":
        raise ValueError("Fill in the Release notes section (or `Internal: ...`)")
    validation = found.get("validation", "")
    if re.search(r"^\s*[-*] \[ \]", validation, re.M):
        raise ValueError("Tick or delete the unchecked Validation items")
    if not re.search(r"^\s*[-*] \[x\]", validation, re.M | re.I):
        raise ValueError("Tick the Validation checks you ran locally")


def codex_reviewed(head, reviews, comments):
    for item in comments + reviews:
        user = item["user"]
        if user["login"] != CODEX:
            continue
        body = item.get("body") or ""
        state = re.search(r"<!-- codex-review-state:v1 (\{.*?\}) -->", body)
        if state:
            review = json.loads(state[1]).get("code_review") or {}
            if (review.get("status") == "completed" and review.get("commit_sha") == head
                    and not review.get("has_unresolved_findings")):
                return True
        commit = re.search(r"\*\*Reviewed commit:\*\*\s*`([0-9a-f]{7,40})`", body)
        if commit and head.startswith(commit[1]) and item.get("state", "COMMENTED") != "DISMISSED":
            return True
    return False


def evaluate(pr, reviews, comments, threads):
    if pr["state"] != "open" or pr["base"]["ref"] != "main":
        return None
    if pr["draft"]:
        return "pending", "Draft PR"
    if pr["user"]["type"] != "Bot":
        try:
            validate_body(pr.get("body"))
        except ValueError as error:
            return "failure", str(error)
    latest = {}
    for review in reviews:
        if review["state"] in {"APPROVED", "CHANGES_REQUESTED", "DISMISSED"}:
            latest[review["user"]["login"]] = review["state"]
    if "CHANGES_REQUESTED" in latest.values():
        return "pending", "Requested changes outstanding"
    if any(not thread["isResolved"] for thread in threads):
        return "pending", "Unresolved review threads"
    if not codex_reviewed(pr["head"]["sha"], reviews, comments):
        return "pending", "Waiting for a completed @codex review of the latest commit"
    return "success", "Codex reviewed the latest commit"


def gh(*args, data=None):
    out = subprocess.run(["gh", "api", *args], input=json.dumps(data) if data else None,
                         text=True, capture_output=True, check=True).stdout
    return json.loads(out) if out.strip() else None


def pages(path):
    return [item for page in gh(path, "--paginate", "--slurp") for item in page]


def review_threads(repo, number):
    owner, name = repo.split("/")
    threads, cursor = [], None
    while True:
        query = ("query($o:String!,$n:String!,$p:Int!,$c:String){repository(owner:$o,name:$n){"
                 "pullRequest(number:$p){reviewThreads(first:100,after:$c){nodes{isResolved}"
                 "pageInfo{hasNextPage endCursor}}}}}")
        args = ["graphql", "-f", f"query={query}", "-F", f"p={number}", "-f", f"o={owner}", "-f", f"n={name}"]
        if cursor:
            args += ["-f", f"c={cursor}"]
        data = gh(*args)["data"]["repository"]["pullRequest"]["reviewThreads"]
        threads += data["nodes"]
        if not data["pageInfo"]["hasNextPage"]:
            return threads
        cursor = data["pageInfo"]["endCursor"]


def check_actions(pr, repo, state, runs):
    """Plan hosted-build changes for the PR head from each workflow's dispatched runs on its branch."""
    if pr["head"]["repo"]["full_name"] != repo:  # fork branches cannot be dispatched
        return []
    sha = pr["head"]["sha"]
    actions = []
    for workflow in HOSTED_CHECKS:
        found = runs.get(workflow, [])
        actions += [("cancel", item["id"]) for item in found
                    if item["head_sha"] != sha and item["status"] in {"queued", "in_progress"}]
        # Any earlier run of this commit, even a failed or cancelled one, blocks a repeat dispatch.
        if state == "success" and not any(item["head_sha"] == sha for item in found):
            actions.append(("dispatch", workflow))
    return actions


def dispatch_payload(branch):
    return {"ref": branch, "inputs": {"os": PR_RUNNER}}


def manage_checks(repo, pr, state):
    branch = pr["head"]["ref"]
    runs = {}
    for workflow in HOSTED_CHECKS:
        query = f"branch={urllib.parse.quote(branch, safe='')}&event=workflow_dispatch&per_page=50"
        found = gh(f"repos/{repo}/actions/workflows/{workflow}/runs?{query}")["workflow_runs"]
        runs[workflow] = [{key: item[key] for key in ("id", "head_sha", "status")} for item in found]
    for action, target in check_actions(pr, repo, state, runs):
        print(f"{action}: {target}")
        if action == "cancel":
            gh(f"repos/{repo}/actions/runs/{target}/cancel", "--method", "POST")
        else:
            gh(f"repos/{repo}/actions/workflows/{target}/dispatches", "--method", "POST",
               "--input", "-", data=dispatch_payload(branch))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", default=os.environ.get("GITHUB_REPOSITORY"))
    parser.add_argument("--pr", type=int, required=True)
    args = parser.parse_args()
    pr = gh(f"repos/{args.repo}/pulls/{args.pr}")
    reviews = pages(f"repos/{args.repo}/pulls/{args.pr}/reviews?per_page=100")
    comments = pages(f"repos/{args.repo}/issues/{args.pr}/comments?per_page=100")
    result = evaluate(pr, reviews, comments, review_threads(args.repo, args.pr))
    if result is None:
        return
    state, description = result
    print(f"{state}: {description}")
    gh(f"repos/{args.repo}/statuses/{pr['head']['sha']}", "--method", "POST", "--input", "-",
       data=dict(state=state, context=CONTEXT, description=description[:140]))
    manage_checks(args.repo, pr, state)


if __name__ == "__main__":
    main()
