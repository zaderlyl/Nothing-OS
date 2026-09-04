#!/bin/bash
# (Re)lance le pont Discord et suit son journal.
#   bridge/run.sh [dossier-partage]     (défaut: ~/Documents)
set -e
cd "$(dirname "$0")"

SHARE="${1:-$HOME/Documents}"
LOG="$HOME/Library/Logs/nothing-bridge.log"

[ -x discord-bridge ] || ./build.sh

pkill -9 -f 'discord-bridge|NothingBridge' 2>/dev/null || true
sleep 1
: > "$LOG"

./discord-bridge "$SHARE" &
BPID=$!
echo "pont lancé (pid $BPID) sur : $SHARE"
echo
echo "  Si « autorisation requise » : coche le Terminal dans"
echo "  Réglages ▸ Confidentialité ▸ Enregistrement de l'écran"
echo "  (+ Accessibilité pour la souris/clavier), le pont la prend"
echo "  en compte tout seul au prochain essai."
echo
echo "  Ctrl-C arrête le pont."
echo
sleep 1
tail -f "$LOG" &
TPID=$!
trap "kill $BPID $TPID 2>/dev/null" EXIT INT TERM
wait $BPID
