#!/bin/bash
# « Faux plein écran » pour Nothing OS (macOS).
#
# QEMU tourne dans une fenêtre NORMALE (pas -full-screen) : ainsi les
# vraies apps du Mac se superposent sans changement de Space. On agrandit
# ensuite la fenêtre pour couvrir l'écran.
#
# macOS ne permet PAS à ce script de masquer la barre de menus / le Dock
# ni de bloquer Cmd-Tab pendant que QEMU est l'app active. Pour ça :
#   bridge/kiosk-lockdown.sh        (affiche / applique les réglages)
#
#   bridge/kiosk.sh [dossier_partagé]
set -e
cd "$(dirname "$0")"

QEMU="${QEMU:-qemu-system-x86_64}"
KERNEL="${KERNEL:-$(cd .. && pwd)/kernel.bin}"
DISK="${DISK:-$(cd .. && pwd)/nothingos.img}"
SHARE="${1:-$HOME/Documents}"
OPENER_LOG="${OPENER_LOG:-/tmp/nothing-opener.log}"
AUDIODEV="${AUDIODEV:-coreaudio,id=snd}"

[ -f "$KERNEL" ] || { echo "kernel absent : $KERNEL — fais 'make'"; exit 1; }

bash opener.sh "$SHARE" > "$OPENER_LOG" 2>&1 &
OPENER=$!
cleanup() { kill "$OPENER" "$QPID" 2>/dev/null || true; }
trap cleanup EXIT INT TERM

"$QEMU" -kernel "$KERNEL" -vga std \
    -display cocoa,zoom-to-fit=on,show-cursor=off \
    -drive file="$DISK",format=raw,if=ide,index=0 \
    -fsdev local,id=fsdev0,path="$SHARE",security_model=none \
    -device virtio-9p-pci,fsdev=fsdev0,mount_tag=hostdocs,disable-modern=on \
    -audiodev "$AUDIODEV" -device AC97,audiodev=snd \
    -serial stdio -m 1G -no-reboot -no-shutdown &
QPID=$!

# fenêtre QEMU : au premier plan + taille écran (best-effort ; demande
# l'autorisation « Accessibilité » du terminal la 1re fois — sinon
# agrandis la fenêtre à la main une fois).
(
  for _ in $(seq 1 20); do
    kill -0 "$QPID" 2>/dev/null || exit 0
    if osascript >/dev/null 2>&1 <<'OSA'
tell application "Finder" to set b to bounds of window of desktop
set SW to item 3 of b
set SH to item 4 of b
tell application "System Events"
  if not (exists process "qemu-system-x86_64") then error "pas encore"
  tell process "qemu-system-x86_64"
    if (count of windows) is 0 then error "pas de fenetre"
    set frontmost to true
    perform action "AXRaise" of window 1
    -- y negatif : la barre de titre « QEMU » passe hors écran, seul le
    -- contenu reste visible (marche si la barre de menus est en
    -- masquage auto : bridge/kiosk-lockdown.sh --apply)
    set position of window 1 to {0, -28}
    set size of window 1 to {SW, (SH + 28)}
  end tell
end tell
OSA
    then exit 0; fi
    sleep 0.5
  done
  echo "kiosk : agrandissement auto impossible — agrandis la fenêtre QEMU à la main (ou autorise « Accessibilité » au terminal)" >&2
) &

wait "$QPID"
