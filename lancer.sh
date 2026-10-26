#!/bin/bash
# Lance Nothing OS dans une fenêtre QEMU (à utiliser avec le kernel.bin
# téléchargé depuis la page Releases, ou compilé avec `make`).
#
#   ./lancer.sh
#   SHARE=~/Music ./lancer.sh    # expose un dossier à l'OS (/doc, /fichier)
set -e
cd "$(dirname "$0")"

KERNEL="${KERNEL:-kernel.bin}"
DISK="${DISK:-nothingos.img}"
QEMU="${QEMU:-qemu-system-x86_64}"

command -v "$QEMU" >/dev/null || {
    echo "QEMU est introuvable. Installe-le :"
    echo "  macOS  : brew install qemu"
    echo "  Linux  : sudo apt install qemu-system-x86"
    exit 1
}
[ -f "$KERNEL" ] || { echo "kernel absent : $KERNEL"; exit 1; }

# disque persistant (16 Mio) : créé au premier lancement
[ -f "$DISK" ] || dd if=/dev/zero of="$DISK" bs=1048576 count=16 2>/dev/null

if [ "$(uname)" = "Darwin" ]; then
    DISPLAY_OPT="-display cocoa,zoom-to-fit=on"
    AUDIO="coreaudio,id=snd"
else
    DISPLAY_OPT=""
    AUDIO="pa,id=snd"
fi

# partage de dossier (optionnel) : virtio-9p, voir le Makefile
SHARE_OPT=()
if [ -n "$SHARE" ]; then
    [ -d "$SHARE" ] || { echo "dossier introuvable : $SHARE"; exit 1; }
    SHARE_OPT=(-fsdev local,id=fsdev0,path="$SHARE",security_model=none
               -device virtio-9p-pci,fsdev=fsdev0,mount_tag=hostdocs,disable-modern=on)
fi

exec "$QEMU" -kernel "$KERNEL" -vga std $DISPLAY_OPT \
    -usb -device usb-tablet \
    -drive file="$DISK",format=raw,if=ide,index=0 \
    "${SHARE_OPT[@]}" \
    -audiodev "$AUDIO" -device AC97,audiodev=snd \
    -m 1G -no-reboot -no-shutdown -rtc base=localtime
