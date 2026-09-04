#!/bin/bash
# Publie l'état du Mac pour la vignette « infos système » (bas-droite)
# de Nothing OS : Wi-Fi, ports branchés, batterie, charge CPU/mémoire.
#
# Écrit <partage>/.nothingos-sys (lignes clef=valeur), relu par le noyau
# (src/sysinfo.rs) via le partage 9p. Aucune donnée sensible, aucune
# autorisation particulière.
#
#   bridge/sysinfo.sh [dossier_partagé]
set -u
SHARE="${1:-$HOME/Documents}"
OUT="$SHARE/.nothingos-sys"
NCPU="$(sysctl -n hw.ncpu 2>/dev/null || echo 8)"
WIFI_IF="$(networksetup -listallhardwareports 2>/dev/null | awk '/Wi-Fi/{getline; print $2; exit}')"
WIFI_IF="${WIFI_IF:-en0}"

usb_cache="—"
usb_at=0

while true; do
    now="$(date +%s)"

    # --- réseau / Wi-Fi ---
    ssid="$(ipconfig getsummary "$WIFI_IF" 2>/dev/null | awk -F' SSID : ' '/ SSID :/{print $2; exit}')"
    wpow="$(networksetup -getairportpower "$WIFI_IF" 2>/dev/null | grep -oE 'On$|Off$')"
    iface="$(route -n get default 2>/dev/null | awk '/interface:/{print $2; exit}')"
    online=0; ping -c1 -W1 1.1.1.1 >/dev/null 2>&1 && online=1

    # --- batterie ---
    battline="$(pmset -g batt 2>/dev/null | grep -E 'InternalBattery' | head -1)"
    blvl="$(printf '%s' "$battline" | grep -oE '[0-9]+%' | head -1 | tr -d '%')"
    bstate="$(printf '%s' "$battline" | grep -oiE 'discharging|charging|charged|AC attached|finishing charge' | head -1)"
    charging=0
    case "$bstate" in charging|charged|"AC attached"|"finishing charge") charging=1 ;; esac

    # --- CPU / mémoire ---
    memfree="$(memory_pressure 2>/dev/null | awk -F': ' '/free percentage/{gsub(/%/,"",$2); print $2; exit}')"
    cpu="$(ps -A -o %cpu= 2>/dev/null | awk -v n="$NCPU" '{s+=$1} END{if(n>0)printf "%d", s/n; else print 0}')"

    # --- ports branchés (USB / adaptateurs) : commande lente → ~15 s ---
    if [ $((now - usb_at)) -ge 15 ]; then
        usb_at="$now"
        u="$(system_profiler SPUSBDataType 2>/dev/null \
              | grep -E '^ {4,10}[A-Za-z].*:$' \
              | sed 's/ *: *$//; s/^ *//' \
              | grep -viE '^(usb.*bus|.*hub$|.*host controller|.*root| *$)' \
              | grep -E '[A-Za-z]{3}' \
              | sort -u | paste -sd '|' - )"
        lan="$(networksetup -listnetworkserviceorder 2>/dev/null \
               | grep -iE 'Hardware Port:.*(USB.*LAN|Ethernet Adapter|Thunderbolt Ethernet)' \
               | sed 's/.*Hardware Port: //; s/,.*//' | head -1)"
        [ -n "$lan" ] && u="${u:+$u|}$lan"
        ext="$(system_profiler SPDisplaysDataType 2>/dev/null | grep -c 'Resolution:')"
        [ "${ext:-0}" -gt 1 ] && u="${u:+$u|}écran externe"
        usb_cache="${u:-—}"
    fi

    {
        echo "wifi=${ssid:-—}"
        echo "wifi_power=${wpow:-?}"
        echo "iface=${iface:-—}"
        echo "online=$online"
        echo "battery=${blvl:-?}"
        echo "charging=$charging"
        echo "batt_state=${bstate:-?}"
        echo "mem_free=${memfree:-?}"
        echo "cpu=${cpu:-0}"
        echo "ports=${usb_cache}"
        echo "host=$(scutil --get ComputerName 2>/dev/null || hostname)"
        echo "ts=$now"
    } > "$OUT.tmp" 2>/dev/null && mv "$OUT.tmp" "$OUT" 2>/dev/null

    sleep 3
done
