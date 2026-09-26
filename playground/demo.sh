#!/usr/bin/env bash
# Self-running demo of the REAL `hippo-task` CLI. No typing, no ids to copy.
#   ./playground/demo.sh          step through it (press enter between beats)
#   ./playground/demo.sh --auto   run start-to-finish with no pauses   (also: make demo)
#
# It drives the actual compiled binary against a fresh scratch store and
# narrates each step, so you can watch every wedge behaviour prove itself.

set -o pipefail
CLI_DIR="$(cd "$(dirname "$0")/.." && pwd)"   # the crate root (this script lives in playground/)
BIN="$CLI_DIR/target/debug/hippo-task"
D="$(mktemp -d 2>/dev/null || echo /tmp/hippo-task-demo)"; rm -rf "$D"; mkdir -p "$D"
AUTO=0; [ "$1" = "--auto" ] && AUTO=1

if [ -t 1 ]; then
  RED=$'\e[31m'; GRN=$'\e[32m'; BLU=$'\e[34m'; MAG=$'\e[35m'; YEL=$'\e[33m'
  DIM=$'\e[2m'; BOLD=$'\e[1m'; RST=$'\e[0m'
else RED=; GRN=; BLU=; MAG=; YEL=; DIM=; BOLD=; RST=; fi

echo "building hippo-task…"; ( cd "$CLI_DIR" && cargo build --locked ) >/dev/null 2>&1 || { echo "build failed — is Rust installed? (make doctor)"; exit 1; }

as(){ local a="$1" n="$2"; shift 2; HIPPO_DIR="$D" HIPPO_ACTOR="$a" HIPPO_NODE="$n" "$BIN" "$@"; }
colorize(){ sed -E -e "s/(\[blocked\])/${RED}\1${RST}/g" -e "s/(lease:[^ ]+)/${MAG}\1${RST}/g" -e "s/(held:[^ ]+)/${MAG}\1${RST}/g" -e "s/(\(rejected\))/${RED}\1${RST}/g" -e "s/ doing / ${BLU}doing${RST} /g" -e "s/ done / ${DIM}done${RST} /g" -e "s/ urgent / ${RED}urgent${RST} /g" -e "s/ high / ${YEL}high${RST} /g"; }
board(){ echo; echo "  ${BOLD}board${RST}"; as human:you n0 list | colorize | sed 's/^/    /'; }
step(){ echo; printf "${BOLD}${BLU}▸ %s${RST}\n" "$*"; }
say(){ printf "  ${DIM}%s${RST}\n" "$*"; }
cmd(){ printf "  ${DIM}\$ %s${RST}\n" "$*"; }
out(){ sed 's/^/    /'; }
pause(){ [ "$AUTO" = 1 ] && { echo; return; }; printf "\n  ${DIM}(enter ▸)${RST}"; read -r _ </dev/tty 2>/dev/null || read -r _ || true; }
num(){ printf '%s' "${1#\#}"; }   # "#3" → "3" (in a shell, '#' would start a comment)
kind(){ case "$1" in 0) printf ok;; 1) printf io;; 2) printf usage;; 3) printf not_found;; 4) printf conflict;; *) printf unknown;; esac; }   # exit code → its kind

clear 2>/dev/null || true
printf "${BOLD}hippo-task — self-running demo${RST}  ${DIM}(the real CLI, on a scratch store)${RST}\n"

step "1 · A human seeds a backlog"
say "Three tasks, created through the real binary. 'add' prints the new task's number."
T1=$(as human:you n0 add "Ship auth"      --priority urgent --label backend | awk '{print $2}')
T2=$(as human:you n0 add "Write docs"     --priority high  | awk '{print $2}')
T3=$(as human:you n0 add "Polish landing" --priority med   | awk '{print $2}')
board; pause

step "2 · Agent 'claude' claims Ship auth and starts it"
cmd "HIPPO_ACTOR=agent:claude HIPPO_NODE=cc  hippo-task start $(num "$T1")"
as agent:claude cc start "$T1" | out
board; pause

step "3 · Agent 'codex' tries the SAME task — the swarm test"
say "A different worker attempts Ship auth. It must be told to back off."
cmd "HIPPO_ACTOR=agent:codex HIPPO_NODE=cx  hippo-task start $(num "$T1")"
as agent:codex cx start "$T1" 2>&1 | out; code=$?
say "exit code $code = $(kind "$code"). The refusal goes to stderr, so a script or agent can branch on it."
say "…so codex takes a free task instead:"
as agent:codex cx start "$T2" | out
board; pause

step "4 · Two agents label the SAME task at once"
say "claude adds 'frontend', codex adds 'review'. With naïve last-writer-wins one would vanish."
as agent:claude cc update "$T3" --label-add frontend >/dev/null
as agent:codex  cx update "$T3" --label-add review   >/dev/null
say "Both survive (set-union):"
as human:you n0 show "$T3" | grep -E '^(title|labels):' | out
pause

step "5 · A dependency: Write docs is blocked by Ship auth"
cmd "hippo-task update $(num "$T2") --block $(num "$T1")"
as human:you n0 update "$T2" --block "$T1" >/dev/null
board
say "Write docs shows [blocked] — computed from the relation, not stored."
pause

step "6 · Only the holder may finish it"
cmd "HIPPO_ACTOR=agent:codex …  hippo-task done $(num "$T1")"
as agent:codex cx done "$T1" 2>&1 | out; code=$?
say "Refused (exit $code = $(kind "$code")). claude, who holds it, finishes it:"
as agent:claude cc done "$T1" | out
board
say "Write docs un-blocked itself the instant its blocker was done."
pause

step "7 · The audit trail — who did what, including what was refused"
cmd "hippo-task show $(num "$T1")"
as human:you n0 show "$T1" | colorize | out
pause

step "8 · What agents read: JSON and exit codes"
cmd "hippo-task show $(num "$T3") --json"
as human:you n0 show "$T3" --json | cut -c1-150 | out
cmd "hippo-task show 99"
as human:you n0 show 99 2>&1 | out; code=$?
say "exit code $code = $(kind "$code"). (0 ok · 1 io · 2 usage · 3 not_found · 4 conflict)"
pause

step "9 · The raw append-only ledger (the source of truth)"
say "Every action above is one immutable line — refused ones too. State is folded from these."
nl -ba "$D/.hippotask/ledger.jsonl" | sed 's/^/    /' | cut -c1-140
echo
printf "${GRN}${BOLD}done.${RST} That was the real binary — same code you'd ship. Scratch store: %s\n" "$D"
