#!/usr/bin/env python3
"""Generate release notes combining RogueTop and upstream PokeRogue changes

Usage: release_notes.py {nightly|versioned}

Writes the rendered markdown body to stdout

Requires GITHUB_TOKEN, GITHUB_REPOSITORY, GITHUB_SHA

Version release notes also requires NEW_VERSION
"""

import json
import os
import sys
import urllib.request

UPSTREAM = "pagefaultgames/pokerogue"
API = "https://api.github.com"


def _request(url: str):
    headers = {
        "Accept": "application/vnd.github+json",
        "Authorization": f"Bearer {os.environ['GITHUB_TOKEN']}",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "roguetop-release-notes",
    }
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers)) as resp:
        return json.loads(resp.read()), resp.headers.get("Link", "")


def gh_list(path: str) -> list:
    """Paginated list endpoint"""
    url = f"{API}/{path.lstrip('/')}"
    results: list = []
    while url:
        data, link = _request(url)
        results.extend(data)
        url = _next_page(link)
    return results


def gh_object(path: str) -> dict:
    """Single-object endpoint, no pagination"""
    data, _ = _request(f"{API}/{path.lstrip('/')}")
    return data


def _next_page(link_header: str) -> str | None:
    for part in link_header.split(","):
        if 'rel="next"' in part:
            return part.split(";")[0].strip().strip("<>")
    return None


def format_commits(commits: list) -> list[str]:
    lines = []
    for c in commits:
        msg = c["commit"]["message"].split("\n", 1)[0]
        lines.append(f"* {msg} ({c['sha'][:7]})")
    return lines or ["No commits"]


def previous_nightly(repo: str) -> dict:
    nightlies = [
        r
        for r in gh_list(f"repos/{repo}/releases?per_page=100")
        if r["tag_name"].startswith("nightly-")
    ]
    nightlies.sort(key=lambda r: r["created_at"], reverse=True)
    return nightlies[0]


def previous_versioned(repo: str, current: str) -> str:
    tagged = [
        r
        for r in gh_list(f"repos/{repo}/releases?per_page=100")
        if not r["tag_name"].startswith("nightly-") and r["tag_name"] != current
    ]
    tagged.sort(key=lambda r: r["created_at"], reverse=True)
    return tagged[0]["tag_name"]


def compare_commits(repo: str, base: str, head: str) -> list:
    return gh_object(f"repos/{repo}/compare/{base}...{head}")["commits"]


def commits_since(repo: str, since: str) -> list:
    return gh_list(f"repos/{repo}/commits?since={since}&per_page=100")


def nightly_body() -> str:
    repo = os.environ["GITHUB_REPOSITORY"]
    head = os.environ["GITHUB_SHA"]
    prev = previous_nightly(repo)

    out = ["## What's Changed in RogueTop", ""]
    out.extend(format_commits(compare_commits(repo, prev["tag_name"], head)))

    out.extend(["", "## What's Changed in PokeRogue", ""])
    out.extend(format_commits(commits_since(UPSTREAM, prev["created_at"])))

    return "\n".join(out) + "\n"


def versioned_body() -> str:
    repo = os.environ["GITHUB_REPOSITORY"]
    head = os.environ["GITHUB_SHA"]
    new_version = os.environ["NEW_VERSION"]
    prev = previous_versioned(repo, new_version)

    out = [
        f"Automated build for PokeRogue {new_version}",
        "",
        "## What's Changed in RogueTop",
        "",
    ]
    out.extend(format_commits(compare_commits(repo, prev, head)))

    pokerogue_header = f"## What's Changed in PokeRogue ({prev} -> {new_version})"
    out.extend(["", pokerogue_header, ""])
    out.extend(format_commits(compare_commits(UPSTREAM, prev, new_version)))

    return "\n".join(out) + "\n"


def main() -> int:
    if len(sys.argv) != 2 or sys.argv[1] not in ("nightly", "versioned"):
        print("usage: release_notes.py {nightly|versioned}", file=sys.stderr)
        return 1
    body = nightly_body() if sys.argv[1] == "nightly" else versioned_body()
    sys.stdout.write(body)
    return 0


if __name__ == "__main__":
    sys.exit(main())
