#!/bin/bash
# Réglages macOS pour un « faux plein écran » vraiment immersif.
#
# macOS n'autorise pas Nothing OS (ni son lanceur) à masquer la barre de
# menus / le Dock ou à bloquer Cmd-Tab pendant que QEMU est l'app active.
# Ce sont des RÉGLAGES UTILISATEUR : ce script te montre lesquels et peut
# les (dés)activer pour toi. Tout est réversible.
#
#   bridge/kiosk-lockdown.sh            # montre l'état + les étapes
#   bridge/kiosk-lockdown.sh --apply    # active le mode immersif
#   bridge/kiosk-lockdown.sh --restore  # remet comme avant
#
# Ce qui reste À FAIRE À LA MAIN (pas d'API fiable) :
#   - Réglages ▸ Bureau et Dock ▸ Raccourci Mission Control / gestes :
#     désactive si tu ne veux pas y accéder.
#   - Cmd-Tab : non désactivable sans logiciel tiers. La commande pour
#     QUITTER Nothing OS reste : dans l'OS, Maj+Tab+Cmd (ou ferme la
#     fenêtre QEMU / Ctrl-C dans le terminal de `make run`).

set -e
mode="${1:-show}"

apply() {
    echo "→ barre de menus : masquage auto"
    defaults write NSGlobalDomain _HIHideMenuBar -bool true
    echo "→ Dock : masquage auto + apparition lente"
    defaults write com.apple.dock autohide -bool true
    defaults write com.apple.dock autohide-delay -float 2
    defaults write com.apple.dock autohide-time-modifier -float 0.4
    echo "→ coins actifs : désactivés"
    for c in tl-corner tr-corner bl-corner br-corner; do
        defaults write com.apple.dock "wvous-$c" -int 1
        defaults write com.apple.dock "wvous-${c%-corner}-modifier" -int 0
    done
    killall Dock 2>/dev/null || true
    echo
    echo "OK. Immersif actif. Reviens avec : bridge/kiosk-lockdown.sh --restore"
}

restore() {
    defaults delete NSGlobalDomain _HIHideMenuBar 2>/dev/null || true
    defaults write com.apple.dock autohide -bool false
    defaults delete com.apple.dock autohide-delay 2>/dev/null || true
    defaults delete com.apple.dock autohide-time-modifier 2>/dev/null || true
    for c in tl-corner tr-corner bl-corner br-corner; do
        defaults delete com.apple.dock "wvous-$c" 2>/dev/null || true
    done
    killall Dock 2>/dev/null || true
    echo "Réglages macOS remis par défaut."
}

show() {
    local mb dock
    mb=$(defaults read NSGlobalDomain _HIHideMenuBar 2>/dev/null || echo 0)
    dock=$(defaults read com.apple.dock autohide 2>/dev/null || echo 0)
    echo "État actuel :"
    echo "  barre de menus masquée auto : $([ "$mb" = 1 ] && echo oui || echo non)"
    echo "  Dock masqué auto            : $([ "$dock" = 1 ] && echo oui || echo non)"
    echo
    echo "Pour le mode immersif :  bridge/kiosk-lockdown.sh --apply"
    echo "Pour revenir en arrière : bridge/kiosk-lockdown.sh --restore"
}

case "$mode" in
    --apply)   apply ;;
    --restore) restore ;;
    *)         show ;;
esac
