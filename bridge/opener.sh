#!/bin/bash
# Ouvre la vraie application du Mac quand Nothing OS le demande.
#
# Nothing OS (src/apps.rs) écrit un mot dans  <partage>/.nothingos-open :
#     vscode | affinity | discord
# suivi d'un numéro de séquence (pour pouvoir ré-ouvrir la même appli).
# Ce script tourne sur le Mac, surveille le fichier et fait `open -a`.
#
# Aucune autorisation spéciale : pas de capture d'écran, pas d'accessibilité.
#
#   bridge/opener.sh                -> partage ~/Documents
#   bridge/opener.sh /autre/dossier -> partage /autre/dossier
set -e

SHARE="$HOME/Documents"
if [ -n "$1" ] && [ "${1#-}" = "$1" ]; then
    case "$1" in
        /*|~*|./*|../*) SHARE="${1/#\~/$HOME}" ;;
        *) echo "argument ignoré : « $1 » (pas un chemin) — partage = $SHARE" ;;
    esac
fi
[ -d "$SHARE" ] || { echo "ERREUR : dossier de partage absent : $SHARE" >&2; exit 1; }

FILE="$SHARE/.nothingos-open"
: > "$FILE"                      # repart propre (on ignore une demande d'avant)
last=""

echo "opener : surveille $FILE"
echo "(Ctrl-C pour arrêter)"

while true; do
    cur="$(sed -n 1p "$FILE" 2>/dev/null | tr -dc 'a-z')"
    arg="$(sed -n 3p "$FILE" 2>/dev/null)"
    stamp="$(cat "$FILE" 2>/dev/null)"
    if [ -n "$cur" ] && [ "$stamp" != "$last" ]; then
        last="$stamp"
        if [ "$cur" = "web" ]; then
            # requête ou URL → navigateur par défaut
            q="$arg"
            if printf %s "$q" | grep -qE '^https?://'; then
                url="$q"
            elif printf %s "$q" | grep -qE '^[A-Za-z0-9._-]+\.[A-Za-z]{2,}(/.*)?$'; then
                url="https://$q"
            else
                url="https://www.google.com/search?q=$(printf %s "$q" | sed 's/ /+/g')"
            fi
            echo "$(date '+%H:%M:%S')  web : $url"
            open "$url"
        else
            case "$cur" in
                vscode|code) app="Visual Studio Code" ;;
                affinity)    app="Affinity" ;;
                discord)     app="Discord" ;;
                claude)      app="Claude" ;;
                spotify)     app="Spotify" ;;
                *)           echo "appli inconnue : « $cur »"; app="" ;;
            esac
            if [ -n "$app" ]; then
                echo "$(date '+%H:%M:%S')  ouvre : $app"
                open -a "$app" || echo "  échec : $app introuvable"
            fi
        fi
    fi
    sleep 0.4
done
