#!/usr/bin/env python3
# Passerelle « Mail » : lit l'app Mail du Mac (AppleScript) et publie les
# messages non lus pour Nothing OS. Gère aussi les actions venant de l'OS
# (ouvrir / marquer lu / archiver).
#
# Fichiers dans <partage>/ :
#   .nothingos-mail        liste : "unread=<n>" puis <id>|<expéditeur>|<sujet>|<date>
#   .nothingos-mail-cmd    l'OS écrit : "<seq> <verbe> <id>"  (open|read|archive)
#   .nothingos-mail-body   corps du message demandé
#
# Aucune donnée ne sort de la machine : on parle seulement à Mail.app en
# local. 1re exécution : macOS demande l'autorisation « Automatisation ».

import os, sys, time, subprocess, unicodedata

SHARE = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/Documents")
LIST = os.path.join(SHARE, ".nothingos-mail")
CMD = os.path.join(SHARE, ".nothingos-mail-cmd")
BODY = os.path.join(SHARE, ".nothingos-mail-body")
PINS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "mail-pins.tsv")
COUNT = 18
REFRESH = 25

last_cmd = ""
last_list = []          # [(id, read, from, subject, date)] du dernier refresh


def load_pins():
    # id \t from \t subject \t date  (méta figée au moment de l'épinglage)
    out = []
    try:
        for ln in open(PINS, encoding="utf-8"):
            p = ln.rstrip("\n").split("\t")
            if len(p) >= 4:
                out.append(p[:4])
    except Exception:
        pass
    return out


def save_pins(pins):
    with open(PINS + ".tmp", "w", encoding="utf-8") as f:
        for p in pins:
            f.write("\t".join(p[:4]) + "\n")
    os.replace(PINS + ".tmp", PINS)


_PUNCT = {"’": "'", "‘": "'", "“": '"', "”": '"',
         "–": "-", "—": "-", "…": "...", " ": " ",
         " ": " ", "‹": "<", "›": ">", "•": "-"}


def ascii_only(s):
    s = s or ""
    for k, v in _PUNCT.items():
        s = s.replace(k, v)
    s = unicodedata.normalize("NFKD", s)
    s = "".join(c for c in s if not unicodedata.combining(c))
    # jette les caractères non-ascii restants (emoji…) au lieu de "?"
    return s.encode("ascii", "ignore").decode("ascii")


def osa(script):
    try:
        r = subprocess.run(["osascript", "-e", script], capture_output=True,
                           text=True, timeout=45)
        if r.returncode != 0 and r.stderr:
            sys.stderr.write("[mail] osascript: " + r.stderr.strip()[:300] + "\n")
        return r.stdout.strip()
    except Exception as e:
        sys.stderr.write(f"[mail] osascript: {e}\n")
        return ""


def refresh_list():
    # `whose read status is false` est trop lent sur une grosse boîte.
    # On lit les COUNT messages les plus récents (boucle bornée, rapide)
    # avec leur statut ; le noyau met en avant les non lus.
    script = f'''
    set AppleScript's text item delimiters to ""
    tell application "Mail"
        set inb to inbox
        set out to "unread=" & (unread count of inb) & linefeed
        repeat with i from 1 to {COUNT}
            try
                set m to message i of inb
                set rr to "0"
                if (read status of m) then set rr to "1"
                set out to out & (id of m) & tab & rr & tab & (sender of m) & tab & (subject of m) & tab & ((date received of m) as string) & linefeed
            end try
        end repeat
        return out
    end tell
    '''
    raw = osa(script)
    if not raw:
        return
    global last_list
    last_list = []
    lines = []
    for ln in raw.splitlines():
        if ln.startswith("unread="):
            lines.append(ln.strip())
            continue
        p = ln.split("\t")
        if len(p) < 5:
            continue
        mid = p[0].strip()
        rd = p[1].strip()
        frm = ascii_only(shorten_sender(p[2]))[:40]
        sub = ascii_only(p[3]).replace("|", "/")[:90] or "(sans sujet)"
        dt = ascii_only(date_label(p[4]))
        last_list.append((mid, rd, frm, sub, dt))
        lines.append(f"{mid}|{rd}|{frm}|{sub}|{dt}")
    # évènements épinglés (méta figée) → lignes PIN|
    for pid, pf, ps, pd in load_pins():
        lines.append(f"PIN|{pid}|{ascii_only(pf)[:40]}|{ascii_only(ps)[:90]}|{ascii_only(pd)}")
    tmp = LIST + ".tmp"
    open(tmp, "w", encoding="utf-8").write("\n".join(lines) + "\n")
    os.replace(tmp, LIST)
    sys.stderr.write(f"[mail] {len(last_list)} msgs, {len(load_pins())} epingles\n")


def shorten_sender(s):
    # "Prénom Nom <x@y>" -> "Prénom Nom" ; sinon l'adresse
    s = (s or "").strip()
    if "<" in s:
        name = s.split("<")[0].strip().strip('"')
        return name or s.split("<")[1].rstrip(">")
    return s


MONTHS = {"January": "janv.", "February": "fevr.", "March": "mars", "April": "avr.",
          "May": "mai", "June": "juin", "July": "juil.", "August": "aout",
          "September": "sept.", "October": "oct.", "November": "nov.", "December": "dec."}


def date_label(s):
    # "Thursday, 4 September 2026 at 13:20:11" -> "4 sept. 13:20" (best effort)
    try:
        parts = s.replace(",", "").split()
        day = parts[1]
        mon = MONTHS.get(parts[2], parts[2][:4].lower())
        tm = ""
        for tok in parts:
            if ":" in tok:
                tm = ":".join(tok.split(":")[:2])
        return f"{day} {mon} {tm}".strip()
    except Exception:
        return s[:16]


def fetch_body(mid):
    script = f'''
    tell application "Mail"
        set m to first message of inbox whose id is {mid}
        set s to ""
        try
            set s to sender of m
        end try
        set sub to ""
        try
            set sub to subject of m
        end try
        set d to ""
        try
            set d to (date received of m) as string
        end try
        set c to ""
        try
            set c to content of m
        end try
        return s & tab & sub & tab & d & tab & tab & c
    end tell
    '''
    raw = osa(script)
    frm, sub, dt, body = "", "", "", ""
    if "\t\t" in raw:
        head, body = raw.split("\t\t", 1)
        hp = head.split("\t")
        frm = hp[0] if len(hp) > 0 else ""
        sub = hp[1] if len(hp) > 1 else ""
        dt = hp[2] if len(hp) > 2 else ""
    body = ascii_only(body)
    # nettoie : lignes vides multiples, espaces en fin
    out = []
    blank = 0
    for ln in body.replace("\r", "").split("\n"):
        ln = ln.rstrip()
        # lignes devenues du bruit après translittération ("?", "? ?", …)
        if ln and not ln.replace("?", "").replace(" ", ""):
            continue
        if not ln:
            blank += 1
            if blank > 1:
                continue
        else:
            blank = 0
        out.append(ln[:200])
        if len(out) > 400:
            break
    txt = "\n".join(out)
    tmp = BODY + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(f"id={mid}\n")
        f.write(f"from={ascii_only(shorten_sender(frm))}\n")
        f.write(f"subject={ascii_only(sub)}\n")
        f.write(f"date={ascii_only(date_label(dt))}\n")
        f.write("---\n")
        f.write(txt + "\n")
    os.replace(tmp, BODY)


def do_cmd(verb, mid):
    if verb == "open":
        fetch_body(mid)
    elif verb == "read":
        osa(f'tell application "Mail" to set read status of (first message of inbox whose id is {mid}) to true')
    elif verb == "archive":
        osa(f'''tell application "Mail"
            set m to first message of inbox whose id is {mid}
            set read status of m to true
            try
                set mailbox of m to (first mailbox of account 1 whose name contains "All Mail")
            on error
                set mailbox of m to (mailbox "Archive" of account 1)
            end try
        end tell''')
    elif verb == "pin":
        pins = load_pins()
        if not any(p[0] == mid for p in pins):
            meta = next((m for m in last_list if m[0] == mid), None)
            if meta:
                pins.insert(0, [mid, meta[2], meta[3], meta[4]])
                save_pins(pins)
            osa(f'tell application "Mail" to set flagged status of (first message of inbox whose id is {mid}) to true')
    elif verb == "unpin":
        save_pins([p for p in load_pins() if p[0] != mid])
        osa(f'try\ntell application "Mail" to set flagged status of (first message of inbox whose id is {mid}) to false\nend try')
    elif verb == "readall":
        # marque lu par lots, EN TÂCHE DE FOND (ne bloque pas la boucle)
        subprocess.Popen(["osascript", "-e", '''
        tell application "Mail"
            repeat with k from 0 to 60
                try
                    set read status of (messages (k * 50 + 1) thru (k * 50 + 50) of inbox) to true
                end try
            end repeat
        end tell'''])
    sys.stderr.write(f"[mail] {verb} {mid}\n")


for f in (LIST, BODY):
    try:
        open(f, "a").close()
    except Exception:
        pass

tick = 0
while True:
    # commandes de l'OS
    try:
        cur = open(CMD, encoding="utf-8").read().strip()
    except Exception:
        cur = ""
    if cur and cur != last_cmd:
        last_cmd = cur
        parts = cur.split()
        if len(parts) >= 3:
            do_cmd(parts[1], parts[2])
            if parts[1] in ("read", "archive"):
                tick = 0  # rafraîchit la liste tout de suite
    if tick <= 0:
        try:
            refresh_list()
        except Exception as e:
            sys.stderr.write(f"[mail] {e}\n")
        tick = REFRESH
    tick -= 1
    time.sleep(1)
