#!/usr/bin/env python3
# Récupère l'agenda (flux iCal) et publie les prochains évènements pour
# le widget « agenda » (bas-gauche) de Nothing OS.
#
# L'URL du flux est lue dans, par ordre de priorité :
#   $CAL_URL   |   bridge/calendar.url   (une URL par ligne, '#' = commentaire)
#
# Écrit <partage>/.nothingos-cal :
#   now=<epoch>
#   <start_epoch>|<end_epoch>|<quand>|<résumé>|<lieu>
#   ... (prochains évènements, triés). <quand> = libellé déjà en heure
#   locale ("auj. 14:00", "lun. 08:00", "23 nov. 15:30").
#
# Aucune donnée sensible ne quitte la machine : on ne fait que des GET
# sur le(s) flux que TU as fournis.

import os, sys, time, calendar, unicodedata, urllib.request, ssl, re


def ascii_only(s):
    # le noyau n'affiche que l'ASCII : on translittère les accents
    for k, v in {"’": "'", "‘": "'", "“": '"', "”": '"', "–": "-",
                 "—": "-", "…": "...", " ": " ", " ": " "}.items():
        s = s.replace(k, v)
    s = unicodedata.normalize("NFKD", s)
    s = "".join(c for c in s if not unicodedata.combining(c))
    return s.encode("ascii", "ignore").decode("ascii")

HERE = os.path.dirname(os.path.abspath(__file__))
SHARE = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/Documents")
OUT = os.path.join(SHARE, ".nothingos-cal")
MAX_EVENTS = 8
REFRESH = 600  # secondes


def urls():
    out = []
    if os.environ.get("CAL_URL"):
        out.append(os.environ["CAL_URL"].strip())
    p = os.path.join(HERE, "calendar.url")
    if os.path.isfile(p):
        for ln in open(p, encoding="utf-8"):
            ln = ln.strip()
            if ln and not ln.startswith("#"):
                out.append(ln)
    return out


def unfold(text):
    # RFC 5545 : une ligne repliée continue par un espace/tab en début
    return re.sub(r"\r?\n[ \t]", "", text)


def parse_dt(v):
    # 20260907T090000Z (UTC) ou 20260907T090000 (local) ou 20260907 (jour)
    v = v.strip()
    m = re.match(r"(\d{4})(\d\d)(\d\d)(?:T(\d\d)(\d\d)(\d\d)(Z)?)?", v)
    if not m:
        return None
    y, mo, d, hh, mm, ss, z = m.groups()
    parts = (int(y), int(mo), int(d), int(hh or 0), int(mm or 0), int(ss or 0), 0, 0, -1)
    if z:  # UTC
        return calendar.timegm(parts)
    return int(time.mktime(parts))  # heure locale


def deescape(s):
    return (s.replace("\\n", " ").replace("\\,", ",")
             .replace("\\;", ";").replace("\\\\", "\\").strip())


def fetch(url):
    ctx = ssl.create_default_context()
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "nothing-os/1.0"})
        with urllib.request.urlopen(req, timeout=25, context=ctx) as r:
            return r.read().decode("utf-8", "replace")
    except Exception as e:
        # repli : contexte non vérifié (certains portails universitaires)
        try:
            ctx2 = ssl._create_unverified_context()
            with urllib.request.urlopen(req, timeout=25, context=ctx2) as r:
                return r.read().decode("utf-8", "replace")
        except Exception as e2:
            sys.stderr.write(f"[calendar] échec {url[:60]}… : {e2}\n")
            return ""


def events_from(ics):
    ics = unfold(ics)
    evs = []
    for block in re.findall(r"BEGIN:VEVENT(.*?)END:VEVENT", ics, re.S):
        d = {}
        for ln in block.splitlines():
            if ":" in ln:
                k, _, val = ln.partition(":")
                k = k.split(";")[0].strip().upper()
                d[k] = val
        s = parse_dt(d.get("DTSTART", ""))
        e = parse_dt(d.get("DTEND", "")) or (s + 3600 if s else None)
        if s is None:
            continue
        desc = deescape(d.get("DESCRIPTION", ""))
        desc = " ".join(desc.split())  # aplatit les retours ligne / espaces
        desc = re.sub(r"\(Exported\s*:[^)]*\)", "", desc).strip()
        evs.append((s, e, deescape(d.get("SUMMARY", "")),
                    deescape(d.get("LOCATION", "")), desc))
    return evs


DAYS = ["lun.", "mar.", "mer.", "jeu.", "ven.", "sam.", "dim."]
MONTHS = ["", "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.",
          "août", "sept.", "oct.", "nov.", "déc."]


def when_label(ep, now):
    lt = time.localtime(ep)
    ln = time.localtime(now)
    hm = time.strftime("%H:%M", lt)
    same_day = lt.tm_yday == ln.tm_yday and lt.tm_year == ln.tm_year
    if same_day:
        return f"auj. {hm}"
    if 0 < (ep - now) < 7 * 86400:
        return f"{DAYS[lt.tm_wday]} {hm}"
    return f"{lt.tm_mday} {MONTHS[lt.tm_mon]} {hm}"


def run_once():
    now = int(time.time())
    allev = []
    for u in urls():
        allev += events_from(fetch(u))
    # prochains évènements (en cours ou à venir), triés
    upc = sorted([e for e in allev if (e[1] or e[0]) >= now])[:MAX_EVENTS]
    lines = [f"now={now}"]
    for s, e, summ, loc, desc in upc:
        summ = ascii_only(summ.replace("|", "/"))[:90]
        loc = ascii_only(loc.replace("|", "/"))[:50]
        desc = ascii_only(desc.replace("|", "/"))[:400]
        when2 = ascii_only(when_label(s, now))
        endlbl = time.strftime("%H:%M", time.localtime(e)) if e else ""
        lines.append(f"{s}|{e or s}|{when2}|{summ}|{loc}|{endlbl}|{desc}")
    tmp = OUT + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write("\n".join(lines) + "\n")
    os.replace(tmp, OUT)
    sys.stderr.write(f"[calendar] {len(upc)} évènements à venir\n")


if not urls():
    sys.stderr.write(
        "[calendar] aucune URL — mets ton lien iCal dans bridge/calendar.url "
        "(ou $CAL_URL). Le widget agenda restera vide.\n")
    # on écrit quand même un fichier vide pour ne pas bloquer le noyau
    open(OUT, "w").write(f"now={int(time.time())}\n")
    sys.exit(0)

while True:
    try:
        run_once()
    except Exception as e:
        sys.stderr.write(f"[calendar] erreur : {e}\n")
    time.sleep(REFRESH)
