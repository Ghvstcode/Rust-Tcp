#!/bin/sh
# Checks that tasks/MANIFEST.lock (at the repository root) lists every
# tasks/*.yaml with the right SHA-256, and nothing else. POSIX sh + sha256sum
# (or shasum).
#
# This is the cheap half of the trust chain: it catches "someone edited a
# manifest and forgot to re-sign". The expensive half - verifying the minisign
# signature against the key the donor pinned - is done by every worker before
# it runs anything, and deliberately not here: CI runs code from the pull
# request, so CI must never be what decides whether that code is trusted.
#
# Skips with a notice when there is nothing to check:
#   - no lock yet (a project that has not been signed for the first time), or
#   - no manifests in the tree (inside a task sandbox they are removed).
set -eu

cd "$(dirname "$0")/../.."

lock="tasks/MANIFEST.lock"

set -- tasks/*.yaml
if [ ! -e "$1" ]; then
    echo "lock-check: no manifests in this tree (task sandbox?) - skipped"
    exit 0
fi
if [ ! -f "$lock" ]; then
    echo "lock-check: NOTICE: $lock does not exist yet - skipped (run 'agentfund sign tasks/')"
    exit 0
fi

if command -v sha256sum >/dev/null 2>&1; then
    sha() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
    sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
    echo "lock-check: neither sha256sum nor shasum found" >&2
    exit 1
fi

# `agentfund sign` writes one manifest object per line group; pull out
# path/sha256 pairs without needing a JSON parser. Tolerates any key order
# and any whitespace, as long as each "path" is followed by its own "sha256"
# (or the other way round) inside one {...} object.
listed="$(tr -d '\n\r\t ' < "$lock" | tr '{' '\n' | sed -n 's/.*"path":"\([^"]*\)".*/\1/p' | sort)"
fail=0

for file in "$@"; do
    object="$(tr -d '\n\r\t ' < "$lock" | tr '{' '\n' | grep -F "\"path\":\"$file\"" || true)"
    if [ -z "$object" ]; then
        echo "lock-check: $file is not listed in $lock" >&2
        fail=1
        continue
    fi
    want="$(printf '%s' "$object" | sed -n 's/.*"sha256":"\([0-9a-f]*\)".*/\1/p')"
    have="$(sha "$file")"
    if [ "$want" != "$have" ]; then
        echo "lock-check: $file has changed since it was signed" >&2
        echo "            lock: $want" >&2
        echo "            file: $have" >&2
        fail=1
    fi
done

for path in $listed; do
    if [ ! -f "$path" ]; then
        echo "lock-check: $lock lists $path, which does not exist" >&2
        fail=1
    fi
done

if [ ! -f "$lock.minisig" ]; then
    echo "lock-check: $lock.minisig is missing (the lock is not signed)" >&2
    fail=1
fi

if [ "$fail" -ne 0 ]; then
    echo "lock-check: FAILED - re-run 'agentfund sign tasks/' and commit the result" >&2
    exit 1
fi
echo "lock-check: OK ($# manifests match $lock)"
