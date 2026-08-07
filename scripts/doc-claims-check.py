"""Check the claims in docs/ that are mechanically checkable.

Run from the repository root:

    python scripts/doc-claims-check.py

Exits 1 if anything looks wrong, 0 otherwise. Written 2026-08-07 after a manual
sweep of this shape found eight defects in one afternoon, including a decision
that contradicted its own standing order and a JSON sample that contradicted the
mechanism the paragraph below it explained.

**This checks the claims that are cheap to verify, not the ones that matter most.**
An output string that no longer exists is caught here; a paragraph that argues
the wrong thing is not, and never will be. `docs/WRITING.md` rule 5 is the rule
this automates one corner of.

**It is deliberately NOT in the pre-commit hook.** The hook runs on every commit
and these checks are worth seconds, not milliseconds; more importantly, a doc
edit mid-thought should not be blocked by a link that will resolve two commits
later. Run it before a review, or when touching a document that quotes output.
"""

import json
import os
import re
import subprocess
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

DOCS = [os.path.join("docs", f) for f in sorted(os.listdir("docs")) if f.endswith(".md")]
DOCS += ["CLAUDE.md"]

# RawGeotag is a separate repository and is not cloned here (2026-08-07). Its paths
# are cited deliberately and resolve nowhere locally, which is correct rather than broken.
REMOTE_REPO_PATHS = {
    "docs/LIGHTROOM-XMP.md",
    "docs/FIXTURES.md",
    "docs/TESTING.md",
    "scripts/verify-fixtures.ps1",
}

failures = []


def report(check, bad, note=""):
    if bad:
        failures.append(check)
        print(f"  [FAIL] {check}{note}")
        for b in bad:
            print(f"           {b}")
    else:
        print(f"  [ ok ] {check}{note}")


def check_links():
    """Every relative markdown link resolves."""
    pattern = re.compile(r"\[[^\]]*\]\(([^)\s#]+)(?:#[^)]*)?\)")
    bad, n = [], 0
    for path in DOCS:
        base = os.path.dirname(path)
        for i, line in enumerate(open(path, encoding="utf-8"), 1):
            for m in pattern.finditer(line):
                target = m.group(1)
                if target.startswith(("http://", "https://", "mailto:")):
                    continue
                n += 1
                resolved = os.path.normpath(os.path.join(base, target))
                if not os.path.exists(resolved):
                    bad.append(f"{path}:{i} -> {target}")
    report("relative markdown links", bad, f" ({n} checked)")


def check_cited_paths():
    """Every file path cited in backticks exists, or is a known remote-repo path."""
    pattern = re.compile(r"`([A-Za-z0-9_./\\-]+\.(?:rs|ps1|py|json|toml|md))`")
    seps = set("/\\")
    bad, cited = [], set()
    for path in DOCS:
        for i, line in enumerate(open(path, encoding="utf-8"), 1):
            for m in pattern.finditer(line):
                raw = m.group(1)
                if not any(c in seps for c in raw):
                    continue
                normalized = raw.replace("\\", "/")
                cited.add(normalized)
                if normalized in REMOTE_REPO_PATHS:
                    continue
                # Resolved relative to the citing document as well as to the root,
                # because `../CLAUDE.md` from docs/ is correct and `crates/...` from
                # anywhere is too. Getting this wrong reports eight defects that are
                # the checker's, which is worse than reporting none.
                candidates = [
                    normalized,
                    os.path.normpath(os.path.join(os.path.dirname(path), normalized)),
                    "crates/offload/" + normalized,
                    "crates/geotag/" + normalized,
                ]
                if not any(os.path.exists(c) for c in candidates):
                    bad.append(f"{path}:{i} -> {raw}")
    report("cited file paths", bad, f" ({len(cited)} distinct)")


def check_invisible_dependencies():
    """No crate is declared in the workspace but imported by nobody.

    Those never resolve, never reach Cargo.lock, and `cargo outdated` cannot see
    them -- so they go stale silently. See docs/TRIP-HYGIENE.md.
    """
    try:
        out = subprocess.run(
            ["cargo", "metadata", "--format-version", "1"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        report("invisible workspace dependencies", [f"cargo metadata failed: {exc}"])
        return
    resolved = {p["name"] for p in json.loads(out)["packages"]}
    manifest = open("Cargo.toml", encoding="utf-8").read()
    section = manifest.split("[workspace.dependencies]")[1].split("\n[")[0]
    declared = {m.group(1) for m in re.finditer(r"^([A-Za-z0-9_-]+)\s*=", section, re.M)}
    report("invisible workspace dependencies", sorted(declared - resolved))


def check_no_red_badge():
    """Decision 14's standing order: no badge in this tool renders red."""
    bad = []
    for root, _, files in os.walk("crates"):
        for f in files:
            if not f.endswith(".rs"):
                continue
            p = os.path.join(root, f)
            for i, line in enumerate(open(p, encoding="utf-8", errors="ignore"), 1):
                if re.search(r"\.red\(\)|\.on_red\(\)", line):
                    bad.append(f"{p}:{i}")
    report("no red badges (decision 14)", bad)


def main():
    if not os.path.isdir("docs"):
        print("Run this from the repository root.")
        return 1

    print("Checking the mechanically checkable claims in docs/ ...\n")
    check_links()
    check_cited_paths()
    check_invisible_dependencies()
    check_no_red_badge()

    print()
    if failures:
        print(f"{len(failures)} check(s) failed.")
        return 1
    print("All checks passed. Note this proves nothing about whether the prose is true --")
    print("only that the claims cheap enough to verify mechanically still hold.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
