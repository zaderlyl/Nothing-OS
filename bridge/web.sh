#!/usr/bin/env python3
# `/web` de Nothing OS.
#
# Le noyau écrit  <partage>/.nothingos-web :  "<seq>\n<requête>\n"
#
#  - si la requête est une QUESTION  -> on cherche une réponse (Wikipédia
#    FR : page la plus pertinente + résumé) et on l'écrit dans
#    .nothingos-web-answer (affichée sous la barre de recherche).
#  - sinon -> on ouvre le navigateur (recherche Google ou URL) et on
#    écrit "open" dans .nothingos-web-answer.
#
# Rien de sensible : requêtes GET publiques + `open`.

import os, sys, re, json, subprocess, time, unicodedata

SHARE = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/Documents")
REQ = os.path.join(SHARE, ".nothingos-web")
ANS = os.path.join(SHARE, ".nothingos-web-answer")

QWORDS = ("qui", "que", "quoi", "quel", "quelle", "quels", "quelles", "quand",
          "comment", "pourquoi", "combien", "ou", "est-ce", "qu'est", "c'est",
          "definition", "signifie", "what", "who", "when", "where", "why",
          "how", "which", "is", "are", "does", "do", "can")


def strip_accents(s):
    s = unicodedata.normalize("NFKD", s or "")
    return "".join(c for c in s if not unicodedata.combining(c))


def ascii_only(s):
    for k, v in {"’": "'", "‘": "'", "“": '"', "”": '"', "–": "-", "—": "-",
                 "…": "...", " ": " ", " ": " "}.items():
        s = s.replace(k, v)
    return strip_accents(s).encode("ascii", "ignore").decode("ascii")


def is_question(q):
    ql = ascii_only(q).strip().lower()
    if ql.endswith("?"):
        return True
    first = re.split(r"[ '’]", ql, maxsplit=1)[0]
    return first in QWORDS


def curl_json(url):
    try:
        out = subprocess.run(["curl", "-sS", "--max-time", "15",
                              "-H", "User-Agent: nothing-os/1.0", url],
                             capture_output=True, text=True, timeout=20).stdout
        return json.loads(out)
    except Exception as e:
        sys.stderr.write(f"[web] {e}\n")
        return None


def wiki_answer(q):
    import urllib.parse
    s = curl_json("https://fr.wikipedia.org/w/api.php?" + urllib.parse.urlencode({
        "action": "query", "list": "search", "srsearch": q,
        "format": "json", "srlimit": "1"}))
    hits = (s or {}).get("query", {}).get("search", [])
    if not hits:
        return None
    title = hits[0]["title"]
    d = curl_json("https://fr.wikipedia.org/api/rest_v1/page/summary/"
                  + urllib.parse.quote(title.replace(" ", "_")))
    extract = (d or {}).get("extract", "")
    if not extract or len(extract) < 20:
        return None
    # 3 phrases max
    parts = re.split(r"(?<=[.!?])\s+", extract)
    txt = " ".join(parts[:3]).strip()
    return title, ascii_only(txt)


def browser(q):
    if re.match(r"^https?://", q):
        url = q
    elif re.match(r"^[A-Za-z0-9._-]+\.[A-Za-z]{2,}(/.*)?$", q):
        url = "https://" + q
    else:
        url = "https://www.google.com/search?q=" + q.replace(" ", "+")
    subprocess.Popen(["open", url])
    return url


def write_ans(text):
    tmp = ANS + ".tmp"
    open(tmp, "w", encoding="utf-8").write(text)
    os.replace(tmp, ANS)


def handle(query):
    query = query.strip()
    if not query:
        return
    if is_question(query):
        res = wiki_answer(query)
        if res:
            title, txt = res
            write_ans(f"q={ascii_only(query)}\nsrc={ascii_only(title)} - Wikipedia\n---\n{txt}\n")
            sys.stderr.write(f"[web] reponse: {title}\n")
            return
        # pas de reponse -> navigateur
    url = browser(query)
    write_ans(f"q={ascii_only(query)}\nopen\n---\n{url}\n")
    sys.stderr.write(f"[web] navigateur: {url}\n")


open(ANS, "w").close()   # repart propre (pas de vieille réponse à l'écran)
try:
    open(REQ, "w").close()
except Exception:
    pass
last = ""
sys.stderr.write(f"[web] surveille {REQ}\n")
while True:
    try:
        cur = open(REQ, encoding="utf-8").read()
    except Exception:
        cur = ""
    if cur and cur != last:
        last = cur
        lines = cur.splitlines()
        if len(lines) >= 2:
            handle(lines[1])
    time.sleep(0.4)
