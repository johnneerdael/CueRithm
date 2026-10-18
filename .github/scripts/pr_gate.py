#!/usr/bin/env python3
"""Lean PR gate: PR body sections plus a completed Codex review of the head commit.

Run only from main with a token that never executes PR code.
"""
import argparse
import json
import os
import re
import subprocess

CONTEXT = "PR gate"
CODEX = "chatgpt-codex-connector[bot]"


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


if __name__ == "__main__":
    main()
