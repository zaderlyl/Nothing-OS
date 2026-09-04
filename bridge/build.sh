#!/bin/bash
# Compile le pont.  On le lance ensuite depuis le Terminal (bridge/run.sh) :
# il hérite alors de l'autorisation « Enregistrement de l'écran » du
# Terminal, donc pas de bidouille de bundle / signature.
set -e
cd "$(dirname "$0")"
swiftc -O discord-bridge.swift -o discord-bridge
echo "construit : $PWD/discord-bridge"
echo "→ lance :  bridge/run.sh"
