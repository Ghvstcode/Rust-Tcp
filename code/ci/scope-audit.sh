#!/bin/sh
# Maintainer tools for the task set. POSIX sh + awk + git. Not part of the gate.
#
#   sh code/ci/scope-audit.sh collisions
#       No two tasks that could be open at the same time may be able to write
#       the same file. Two tasks are "ordered" when one depends on the other,
#       directly or transitively; unordered tasks must have disjoint scopes.
#
#   sh code/ci/scope-audit.sh layers
#       Print the dependency DAG layer by layer (how wide the parallelism is).
#
#   sh code/ci/scope-audit.sh table
#       Print id, max_tokens, depends_on and scope.write for every task.
#
#   sh code/ci/scope-audit.sh diff <task-id> <base-rev> [<head-rev>]
#       Every path changed between the two revisions must be in the task's
#       scope.write (what an AgentFund worker enforces before it reports).
#
# These read the manifests with awk, which works because these manifests
# stick to one layout: `scope.write` and `depends_on` are block lists of plain
# scalars, one per line, and write scopes are literal paths (no globs). The
# real schema validation is `agentfund validate tasks/`.
set -eu

cd "$(dirname "$0")/../.."

# Emits "<id> W <path>" and "<id> D <dep>" and "<id> T <max_tokens>" lines.
facts() {
    for file in tasks/*.yaml; do
        awk '
            /^id:/              { id = $2 }
            /^max_tokens:/      { tokens = $2 }
            /^scope:/           { section = "scope"; next }
            /^depends_on:/      { section = "deps"; if ($2 == "[]") section = ""; next }
            /^[a-z_]+:/         { section = "" }
            section == "scope" && /^  write:/ { sub_section = "write"; next }
            section == "scope" && /^  read:/  { sub_section = "read"; next }
            section == "scope" && sub_section == "write" && /^    - / { w[++nw] = $2 }
            section == "deps" && /^  - /      { d[++nd] = $2 }
            END {
                print id, "T", tokens
                for (i = 1; i <= nw; i++) print id, "W", w[i]
                for (i = 1; i <= nd; i++) print id, "D", d[i]
            }
        ' "$file"
    done
}

# Reads facts; prints "<a> <b>" for every pair where a transitively depends on b.
closure() {
    awk '
        $2 == "T" { ids[$1] = 1 }
        $2 == "D" { dep[$1, $3] = 1 }
        END {
            changed = 1
            while (changed) {
                changed = 0
                for (a in ids) for (b in ids) if (dep[a, b]) for (c in ids)
                    if (dep[b, c] && !dep[a, c]) { dep[a, c] = 1; changed = 1 }
            }
            for (a in ids) for (b in ids) if (dep[a, b]) print a, b
        }
    '
}

case "${1:-}" in
collisions)
    facts > "${TMPDIR:-/tmp}/tcp-facts.$$"
    closure < "${TMPDIR:-/tmp}/tcp-facts.$$" > "${TMPDIR:-/tmp}/tcp-closure.$$"
    status=0
    awk -v closure="${TMPDIR:-/tmp}/tcp-closure.$$" '
        BEGIN { while ((getline line < closure) > 0) { split(line, pr, " "); ordered[pr[1], pr[2]] = 1; ordered[pr[2], pr[1]] = 1 } }
        $2 == "T" { n++ }
        $2 == "D" { deps[$3] = 1 }
        $2 == "T" { seen_id[$1] = 1 }
        $2 == "W" {
            if ($3 ~ /[*?{\[]/) { printf "GLOB      %s writes %s (this audit only understands literal paths)\n", $1, $3; bad = 1 }
            if ($3 ~ /^(agentfund\.yaml$|tasks\/|\.github\/)/) { printf "FORBIDDEN %s writes %s (never writable)\n", $1, $3; bad = 1 }
            for (i = 1; i <= count[$3]; i++) {
                other = owner[$3, i]
                pairs++
                if (ordered[$1, other]) { printf "ok        %-34s shared by %s and %s (ordered by depends_on)\n", $3, other, $1 }
                else { printf "COLLISION %-34s writable by %s and %s, which can be open at the same time\n", $3, other, $1; bad = 1 }
            }
            owner[$3, ++count[$3]] = $1
            paths[$3] = 1
        }
        END {
            for (d in deps) if (!(d in seen_id)) { printf "MISSING   depends_on names unknown task %s\n", d; bad = 1 }
            np = 0; for (q in paths) np++
            printf "%d tasks, %d writable paths, %d shared; %s\n", n, np, pairs, bad ? "FAILED" : "no two concurrently-open tasks can write the same file"
            exit bad
        }
    ' "${TMPDIR:-/tmp}/tcp-facts.$$" || status=$?
    # a dependency cycle would make every task in it unclaimable forever
    if awk '$1 == $2 { found = 1 } END { exit !found }' "${TMPDIR:-/tmp}/tcp-closure.$$"; then
        echo "CYCLE     depends_on contains a cycle" >&2
        status=1
    fi
    rm -f "${TMPDIR:-/tmp}/tcp-facts.$$" "${TMPDIR:-/tmp}/tcp-closure.$$"
    exit "$status"
    ;;
layers)
    facts | awk '
        $2 == "T" { ids[$1] = 1; n++; tokens[$1] = $3 }
        $2 == "D" { dep[$1, $3] = 1 }
        END {
            placed = 0; layer = 0
            while (placed < n) {
                count = 0; line = ""; sum = 0
                for (a in ids) if (!(a in at)) {
                    ready = 1
                    for (b in ids) if (dep[a, b] && (!(b in at) || at[b] == layer)) ready = 0
                    if (ready) pick[++count] = a
                }
                if (count == 0) { print "cycle in depends_on"; exit 1 }
                # sort the layer for stable output
                for (i = 1; i <= count; i++) for (j = i + 1; j <= count; j++) if (pick[j] < pick[i]) { t = pick[i]; pick[i] = pick[j]; pick[j] = t }
                for (i = 1; i <= count; i++) { at[pick[i]] = layer; line = line " " pick[i]; sum += tokens[pick[i]]; placed++ }
                printf "layer %d  width %2d  %5.1fM tokens :%s\n", layer, count, sum / 1000000, line
                total += sum; layer++
            }
            printf "%d tasks in %d layers, %.1fM tokens in total\n", n, layer, total / 1000000
        }
    '
    ;;
table)
    facts | awk '
        $2 == "T" { order[++n] = $1; tokens[$1] = $3 }
        $2 == "W" { w[$1] = w[$1] (w[$1] ? ", " : "") $3 }
        $2 == "D" { d[$1] = d[$1] (d[$1] ? ", " : "") $3 }
        END { for (i = 1; i <= n; i++) { id = order[i]; printf "%s | %s | %s | %s\n", id, tokens[id], (d[id] ? d[id] : "-"), w[id] } }
    '
    ;;
diff)
    id="${2:?usage: scope-audit.sh diff <task-id> <base-rev> [<head-rev>]}"
    base="${3:?usage: scope-audit.sh diff <task-id> <base-rev> [<head-rev>]}"
    head="${4:-HEAD}"
    [ -f "tasks/$id.yaml" ] || { echo "no such task: $id" >&2; exit 1; }
    allowed="$(facts | awk -v id="$id" '$1 == id && $2 == "W" { print $3 }')"
    bad=0
    # --no-renames: a rename is a delete plus an add, and both sides must be in scope
    for path in $(git diff --no-renames --name-only "$base" "$head"); do
        ok=0
        for a in $allowed; do [ "$a" = "$path" ] && ok=1; done
        case "$path" in agentfund.yaml|tasks/*|.github/*) ok=0 ;; esac
        if [ "$ok" -eq 1 ]; then echo "in scope      $path"; else echo "OUT OF SCOPE  $path"; bad=1; fi
    done
    [ "$bad" -eq 0 ] && echo "scope-audit: $id stayed inside its write scope ($base..$head)" || { echo "scope-audit: $id FAILED" >&2; exit 1; }
    ;;
*)
    sed -n '2,24p' "$0" | sed 's/^# \{0,1\}//'
    exit 2
    ;;
esac
