#!/bin/bash
# (Re)lance le pont et suit son journal.
#   bridge/run.sh [dossier-partage]   (défaut: ~/Documents)
set -e
cd "$(dirname "$0")"

SHARE="${1:-$HOME/Documents}"
LOG="$HOME/Library/Logs/nothing-bridge.log"

[ -x NothingBridge.app/Contents/MacOS/NothingBridge ] || ./build.sh

pkill -f NothingBridge.app 2>/dev/null || true
sleep 1
: > "$LOG"
open NothingBridge.app --args "$SHARE"

echo "pont lancé sur : $SHARE"
echo "journal ($LOG) — Ctrl-C pour arrêter le suivi (le pont continue) :"
echo
sleep 1
tail -f "$LOG"
