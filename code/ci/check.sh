#!/bin/sh
# CI gate for the code companion. POSIX sh, no dependencies beyond cargo and
# coreutils. Runs from anywhere; paths below are relative to code/.
#
#   sh code/ci/check.sh              what CI runs: build, unit tests, clippy, and
#                                    the acceptance test of every task marked done
#   sh code/ci/check.sh <task-id>    what a task's `verify` runs: all of the
#                                    above, plus that task's acceptance test, plus
#                                    "did you leave your marker?"
#
# How "done" works: a task is done when the empty file code/ci/done/<task-id> exists.
# Each task creates exactly its own marker (it is in its write scope), so two
# tasks never touch the same file. From then on its acceptance test is part of
# the gate for every later pull request: a regression gate that grows by itself.
#
# This script, the acceptance tests, Cargo.toml and the workflow that calls it
# are outside every task's write scope. An agent cannot make its task pass by
# editing the judge.
#
# Environment:
#   TCP_CLIPPY=off       skip clippy even if it is installed
set -eu

cd "$(dirname "$0")/.."

task="${1:-}"
CARGO="cargo"
FLAGS="--locked --offline"

say() { printf '\n==> %s\n' "$*"; }
die() { printf 'check.sh: %s\n' "$*" >&2; exit 1; }

# 0004-final-handshake -> t0004_final_handshake
target_of() { printf 't%s' "$1" | tr '-' '_'; }

known_task() { [ -f "acceptance/$(target_of "$1").rs" ]; }

# --- 1. the task set itself -------------------------------------------------
say "manifest lock"
sh ci/lock-check.sh

# --- 2. markers are sane ----------------------------------------------------
say "markers"
done_tasks=""
for marker in ci/done/*; do
    [ -e "$marker" ] || continue
    id="$(basename "$marker")"
    [ -f "$marker" ] && [ ! -L "$marker" ] || die "ci/done/$id must be a regular file"
    [ ! -s "$marker" ] || die "ci/done/$id must be empty (it is a marker, not a place to put things)"
    known_task "$id" || die "ci/done/$id does not name a task (no acceptance/$(target_of "$id").rs)"
    done_tasks="$done_tasks $id"
done
if [ -n "$task" ]; then
    known_task "$task" || die "unknown task '$task' (no acceptance/$(target_of "$task").rs)"
    [ -f "ci/done/$task" ] || die "task '$task' has not created its marker file ci/done/$task"
fi
printf 'done:%s\n' "${done_tasks:- (none yet)}"

# --- 3. build, unit tests, lints --------------------------------------------
say "cargo build"
$CARGO build $FLAGS

say "cargo test --lib (unit tests written by tasks, next to their code)"
$CARGO test $FLAGS --lib

# Every acceptance target must still compile: the public signatures in the
# skeleton are a contract between tasks, and this is what enforces it.
say "contract: every acceptance target compiles"
$CARGO test $FLAGS --no-run

if [ "${TCP_CLIPPY:-on}" = "off" ]; then
    say "clippy: skipped (TCP_CLIPPY=off)"
elif $CARGO clippy --version >/dev/null 2>&1; then
    # Default lint levels on purpose: only deny-by-default (correctness) lints
    # fail the gate. A sandbox without clippy must not be able to produce work
    # that a CI with clippy then rejects over style.
    say "cargo clippy"
    $CARGO clippy $FLAGS --all-targets
else
    say "clippy: not installed, skipped"
fi

# --- 4. acceptance tests ----------------------------------------------------
# Marked tasks, plus the task being verified (already required to be marked).
failed=""
for id in $done_tasks; do
    say "acceptance: $id"
    if ! $CARGO test $FLAGS --test "$(target_of "$id")"; then
        failed="$failed $id"
    fi
done

if [ -n "$failed" ]; then
    printf '\ncheck.sh: FAILED acceptance:%s\n' "$failed" >&2
    exit 1
fi

if [ -n "$task" ]; then
    printf '\ncheck.sh: OK - task %s passes, and so does everything merged before it\n' "$task"
else
    printf '\ncheck.sh: OK\n'
fi
