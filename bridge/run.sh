#!/bin/bash
# (Re)lance le pont Discord et suit son journal.
#   bridge/run.sh                 -> partage ~/Documents
#   bridge/run.sh /autre/dossier  -> partage /autre/dossier
set -e
cd "$(dirname "$0")"

# argument = dossier partagé, mais on ignore tout ce qui n'est pas un
# vrai chemin (ex. un « # commentaire » collé par erreur)
SHARE="$HOME/Documents"
if [ -n "$1" ] && [ "${1#-}" = "$1" ] && [ "${1#\#}" = "$1" ]; then
    case "$1" in
        /*|~*|./*|../*) SHARE="${1/#\~/$HOME}" ;;
        *) echo "argument ignoré : « $1 » (pas un chemin) — partage = $SHARE" ;;
    esac
fi

if [ ! -d "$SHARE" ]; then
    echo "ERREUR : le dossier de partage n'existe pas : $SHARE" >&2
    exit 1
fi

LOG="$HOME/Library/Logs/nothing-bridge.log"
[ -x discord-bridge ] || ./build.sh

# tue TOUT ancien pont, puis vérifie
pkill -9 -f "$PWD/discord-bridge" 2>/dev/null || true
pkill -9 -f 'bridge/discord-bridge' 2>/dev/null || true
sleep 1
while pgrep -f 'discord-bridge' >/dev/null; do
    echo "attente arrêt d'un ancien pont…"; pkill -9 -f 'discord-bridge' || true; sleep 1
done

: > "$LOG"
./discord-bridge "$SHARE" &
BPID=$!
tail -f "$LOG" &
TPID=$!
trap 'kill $BPID $TPID 2>/dev/null' EXIT INT TERM

echo "pont lancé (pid $BPID) — partage : $SHARE"
echo "les images vont dans : $SHARE/.nothingos-bridge/"
echo "(Ctrl-C pour arrêter)"
echo
wait $BPID
