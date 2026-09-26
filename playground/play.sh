#!/usr/bin/env bash
# Terminal playground for the REAL `hippo-task` CLI.
# Runs the compiled binary in a loop, redraws a colorized board after every
# command, and lets you switch identity to test multi-agent contention.
#
#   ./playground/play.sh                # uses playground/play-data as a scratch store
#   ./playground/play.sh /path/to/repo  # drive the .hippotask/ ledger in a real repo instead
#
# At the prompt, type any real CLI command (add / list / show / update / lease /
# start / release / note / desc / done) OR a playground helper:
#   who · seed · ledger · hist <id> · help · q

set -o pipefail
CLI_DIR="$(cd "$(dirname "$0")/.." && pwd)"   # the crate root (this script lives in playground/)
BIN="$CLI_DIR/target/debug/hippo-task"
DATADIR="${1:-$CLI_DIR/playground/play-data}"
PACT="human:you"; PNODE="n0"

if [ -t 1 ]; then
  RED=$'\e[31m'; GRN=$'\e[32m'; BLU=$'\e[34m'; MAG=$'\e[35m'; YEL=$'\e[33m'
  DIM=$'\e[2m'; BOLD=$'\e[1m'; RST=$'\e[0m'
else RED=; GRN=; BLU=; MAG=; YEL=; DIM=; BOLD=; RST=; fi

echo "building hippo-task…"; ( cd "$CLI_DIR" && cargo build --locked ) >/dev/null 2>&1 || { echo "build failed — is Rust installed? (make doctor)"; exit 1; }
mkdir -p "$DATADIR"
LASTMSG="ready — type  seed  for a demo, or  add \"My task\" --priority high"

colorize(){
  sed -E \
    -e "s/(\[blocked\])/${RED}\1${RST}/g" \
    -e "s/(lease:[^ ]+)/${MAG}\1${RST}/g" \
    -e "s/ doing / ${BLU}doing${RST} /g" \
    -e "s/ done / ${DIM}done${RST} /g" \
    -e "s/ urgent / ${RED}urgent${RST} /g" \
    -e "s/ high / ${YEL}high${RST} /g"
}

board(){
  [ -t 1 ] && clear
  printf "${BOLD}hippo-task playground${RST}   acting as ${MAG}%s${RST} · node ${MAG}%s${RST}\n" "$PACT" "$PNODE"
  printf "${DIM}store: %s${RST}\n" "$DATADIR/.hippotask/ledger.jsonl"
  printf '%s\n' "────────────────────────────────────────────────────────────"
  local out; out="$("$BIN" --dir "$DATADIR" list 2>/dev/null)"
  if [ -z "$out" ]; then printf "  ${DIM}(no tasks yet)${RST}\n"; else printf '%s\n' "$out" | colorize; fi
  printf '%s\n' "────────────────────────────────────────────────────────────"
  [ -n "$LASTMSG" ] && printf "› %s\n" "$LASTMSG"
  printf "${DIM}cli: add list show update lease start release note desc done   helpers: who · seed · ledger · hist <id> · help · q${RST}\n"
}

seed(){
  rm -rf "$DATADIR"; mkdir -p "$DATADIR"
  local t1 t2
  t1="$(HIPPO_ACTOR=human:you HIPPO_NODE=n0 "$BIN" --dir "$DATADIR" add "Ship auth"  --priority urgent --label backend | awk '{print $2}')"
  t2="$(HIPPO_ACTOR=human:you HIPPO_NODE=n0 "$BIN" --dir "$DATADIR" add "Write docs" --priority high | awk '{print $2}')"
  HIPPO_ACTOR=human:you    HIPPO_NODE=n0 "$BIN" --dir "$DATADIR" add "Polish landing" --priority med >/dev/null
  HIPPO_ACTOR=agent:claude HIPPO_NODE=cc "$BIN" --dir "$DATADIR" start "$t1" >/dev/null
  HIPPO_ACTOR=agent:codex  HIPPO_NODE=cx "$BIN" --dir "$DATADIR" lease "$t2" >/dev/null
  HIPPO_ACTOR=human:you    HIPPO_NODE=n0 "$BIN" --dir "$DATADIR" update "$t2" --block "$t1" >/dev/null
  LASTMSG="seeded. Ship auth is claude's. Try:  who agent:codex cx  →  start ${t1#\#}   (you'll be told to back off)"
}

helptext(){
  cat <<EOF

  Any real CLI command works verbatim, e.g.:
    add "Ship auth" --priority high --label backend
    start 1               (id = the number on the board; '#1' works here too)
    update 1 --state doing --label-add urgent --block 2
    note 1 "left off at the token refresh"
    done 1                (refused if another worker holds the lease; --force overrides)

  Playground helpers:
    who                   switch identity (actor + node) — the multi-agent test
    who <actor> <node>
    seed                  load a demo backlog with a live lease conflict
    ledger                show the raw append-only event ledger
    hist <id>             show one task's full history
    q                     quit
EOF
  read -rp "  (enter to continue) " _
}

while true; do
  export HIPPO_DIR="$DATADIR" HIPPO_ACTOR="$PACT" HIPPO_NODE="$PNODE"
  board
  printf "\n${BOLD}%s@%s${RST}> " "$PACT" "$PNODE"
  IFS= read -r line || break
  # In a shell '#1' would start a comment; here we accept it and mean task 1.
  line="$(printf '%s' "$line" | sed -E 's/(^|[[:space:]])#([0-9]+)/\1\2/g')"
  case "$line" in
    q|quit|exit) break ;;
    ""|ls|board) LASTMSG="" ;;
    help) helptext ;;
    who) read -rp "  actor: " a; read -rp "  node:  " n; [ -n "$a" ] && PACT="$a"; [ -n "$n" ] && PNODE="$n"; LASTMSG="now acting as $PACT@$PNODE" ;;
    who\ *) set -- $line; [ -n "$2" ] && PACT="$2"; [ -n "$3" ] && PNODE="$3"; LASTMSG="now acting as $PACT@$PNODE" ;;
    seed) seed ;;
    ledger) [ -t 1 ] && clear; echo "── event ledger ──"; nl -ba "$DATADIR/.hippotask/ledger.jsonl" 2>/dev/null || echo "(empty)"; read -rp "(enter) " _ ;;
    hist\ *) [ -t 1 ] && clear; "$BIN" --dir "$DATADIR" show "${line#hist }"; read -rp "(enter) " _ ;;
    *) LASTMSG="$(eval "\"$BIN\" --dir \"$DATADIR\" $line" 2>&1)"; code=$?
       [ "$code" -ne 0 ] && LASTMSG="${RED}${LASTMSG}${RST}  ${DIM}(exit $code)${RST}" ;;
  esac
done
echo "bye — your ledger is at $DATADIR/.hippotask/ledger.jsonl"
