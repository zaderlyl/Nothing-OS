#!/bin/bash
# Lance l'« opener » : il ouvre la vraie appli du Mac quand Nothing OS
# le demande (/app dans l'OS).
#
#   bridge/run.sh                 -> partage ~/Documents
#   bridge/run.sh /autre/dossier  -> partage /autre/dossier
set -e
cd "$(dirname "$0")"
chmod +x opener.sh
exec ./opener.sh "$@"
