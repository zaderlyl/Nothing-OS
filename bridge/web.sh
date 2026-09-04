#!/usr/bin/env python3
# `/web` de Nothing OS — voir bridge/README.md pour le protocole complet.
#
#   .nothingos-web          <seq>\n<requête ou url>\n   (écrit par l'OS)
#   .nothingos-web-answer   réponse directe (question -> Wikipédia)
#   .nothingos-web-results  liste de résultats (recherche générale, si
#                           GOOGLE_CSE_KEY/CX configurés dans websearch.env)
#   .nothingos-web-open     <seq>\n<url>\n   (l'OS demande l'ouverture
#                           d'un résultat / lien — clic dans la liste ou
#                           dans un article)
#   .nothingos-web-article  article extrait (titre + paragraphes + liens),
#                           affiché nativement si "lisible" ; sinon on
#                           bascule sur Firefox.
#
# Aucune clé n'est manipulée par Claude : websearch.env est créé par
# l'utilisateur lui-même (voir README), local, jamais commité.

import os, sys, re, json, subprocess, time, unicodedata
from html.parser import HTMLParser

HERE = os.path.dirname(os.path.abspath(__file__))
SHARE = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/Documents")
REQ = os.path.join(SHARE, ".nothingos-web")
OPEN_REQ = os.path.join(SHARE, ".nothingos-web-open")
FIREFOX_REQ = os.path.join(SHARE, ".nothingos-web-firefox")
ANS = os.path.join(SHARE, ".nothingos-web-answer")
RESULTS = os.path.join(SHARE, ".nothingos-web-results")
ARTICLE = os.path.join(SHARE, ".nothingos-web-article")
CONFIG = os.path.join(HERE, "websearch.env")

QWORDS = ("qui", "que", "quoi", "quel", "quelle", "quels", "quelles", "quand",
          "comment", "pourquoi", "combien", "ou", "est-ce", "qu'est", "c'est",
          "definition", "signifie", "what", "who", "when", "where", "why",
          "how", "which", "is", "are", "does", "do", "can")

MIN_WORDS = 80  # sous ce seuil, un article est jugé "pas lisible nativement"


def strip_accents(s):
    s = unicodedata.normalize("NFKD", s or "")
    return "".join(c for c in s if not unicodedata.combining(c))


def ascii_only(s):
    for k, v in {"’": "'", "‘": "'", "“": '"', "”": '"', "–": "-", "—": "-",
                 "…": "...", " ": " ", " ": " "}.items():
        s = s.replace(k, v)
    return strip_accents(s).encode("ascii", "ignore").decode("ascii")


def is_url(q):
    return bool(re.match(r"^https?://", q) or
                re.match(r"^[A-Za-z0-9._-]+\.[A-Za-z]{2,}(/.*)?$", q))


def to_url(q):
    if re.match(r"^https?://", q):
        return q
    return "https://" + q


def is_question(q):
    ql = ascii_only(q).strip().lower()
    if ql.endswith("?"):
        return True
    first = re.split(r"[ '’]", ql, maxsplit=1)[0]
    return first in QWORDS


def load_config():
    cfg = {}
    try:
        for ln in open(CONFIG, encoding="utf-8"):
            ln = ln.strip()
            if ln and not ln.startswith("#") and "=" in ln:
                k, v = ln.split("=", 1)
                cfg[k.strip()] = v.strip()
    except Exception:
        pass
    return cfg


def curl_json(url):
    try:
        out = subprocess.run(["curl", "-sS", "--max-time", "15",
                              "-H", "User-Agent: nothing-os/1.0", url],
                             capture_output=True, text=True, timeout=20).stdout
        return json.loads(out)
    except Exception as e:
        sys.stderr.write(f"[web] {e}\n")
        return None


def curl_html(url):
    try:
        return subprocess.run(
            ["curl", "-sSL", "--max-time", "15", "-A",
             "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15) NothingOS/1.0",
             url],
            capture_output=True, text=True, timeout=25).stdout
    except Exception as e:
        sys.stderr.write(f"[web] fetch: {e}\n")
        return ""


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
    parts = re.split(r"(?<=[.!?])\s+", extract)
    txt = " ".join(parts[:3]).strip()
    return title, ascii_only(txt)


def google_search(q):
    cfg = load_config()
    key, cx = cfg.get("GOOGLE_CSE_KEY"), cfg.get("GOOGLE_CSE_CX")
    if not key or not cx:
        return None
    import urllib.parse
    url = "https://www.googleapis.com/customsearch/v1?" + urllib.parse.urlencode(
        {"key": key, "cx": cx, "q": q, "num": 8})
    d = curl_json(url)
    if not d:
        return None
    if "error" in d:
        sys.stderr.write(f"[web] Google CSE: {d['error'].get('message','?')}\n")
        return None
    items = d.get("items", [])
    return [(it.get("title", ""), it.get("link", ""), it.get("snippet", ""))
            for it in items[:8]]


# --- extraction « lisible » (sans dépendance externe) -------------------
class Extractor(HTMLParser):
    SKIP = {"script", "style", "nav", "header", "footer", "aside",
            "noscript", "svg", "form", "button", "iframe", "template"}
    BREAK = {"p", "li", "h1", "h2", "h3", "h4", "div", "br", "article", "section"}

    def __init__(self):
        super().__init__()
        self.skip_depth = 0
        self.paras = []
        self.cur = []
        self.title = ""
        self.in_title = False

    def handle_starttag(self, tag, attrs):
        if tag in self.SKIP:
            self.skip_depth += 1
        elif tag == "title":
            self.in_title = True
        elif tag in self.BREAK and self.cur:
            self.paras.append(" ".join(self.cur).strip())
            self.cur = []

    def handle_endtag(self, tag):
        if tag in self.SKIP and self.skip_depth > 0:
            self.skip_depth -= 1
        elif tag == "title":
            self.in_title = False
        elif tag in self.BREAK and self.cur:
            self.paras.append(" ".join(self.cur).strip())
            self.cur = []

    def handle_data(self, data):
        if self.skip_depth:
            return
        if self.in_title:
            self.title += data
            return
        d = data.strip()
        if d:
            self.cur.append(d)


def extract_article(url):
    html = curl_html(url)
    if not html:
        return None
    p = Extractor()
    try:
        p.feed(html)
    except Exception:
        pass
    if p.cur:
        p.paras.append(" ".join(p.cur))
    paras = [re.sub(r"\s+", " ", x).strip() for x in p.paras]
    paras = [x for x in paras if len(x.split()) >= 6]
    # dédoublonne les lignes de nav/menus répétées
    seen = set()
    uniq = []
    for x in paras:
        if x not in seen:
            uniq.append(x)
            seen.add(x)
    wc = sum(len(x.split()) for x in uniq)
    if wc < MIN_WORDS:
        return None
    title = ascii_only(p.title.strip())[:120] or url
    body = [ascii_only(x)[:400] for x in uniq[:80]]
    return title, body


def write(path, text):
    tmp = path + ".tmp"
    open(tmp, "w", encoding="utf-8").write(text)
    os.replace(tmp, path)


def open_in_firefox(url):
    if os.path.isdir("/Applications/Firefox.app"):
        subprocess.Popen(["open", "-a", "Firefox", url])
    else:
        subprocess.Popen(["open", url])


def handle_open(url):
    """Un résultat / lien a été cliqué dans l'OS : essaie l'extraction,
    sinon bascule sur le navigateur."""
    write(ANS, "")
    write(RESULTS, "")
    res = extract_article(url)
    if res:
        title, body = res
        write(ARTICLE, f"url={url}\ntitle={title}\n---\n" + "\n".join(body) + "\n")
        sys.stderr.write(f"[web] article lisible : {title} ({len(body)} paragraphes)\n")
    else:
        write(ARTICLE, "")
        open_in_firefox(url)
        write(ANS, f"q=\nopen\n---\n{url}\n")
        sys.stderr.write(f"[web] pas lisible -> Firefox : {url}\n")


def handle_search(query):
    query = query.strip()
    if not query:
        return
    write(ARTICLE, "")
    write(RESULTS, "")

    if is_url(query):
        handle_open(to_url(query))
        return

    if is_question(query):
        res = wiki_answer(query)
        if res:
            title, txt = res
            write(ANS, f"q={ascii_only(query)}\nsrc={ascii_only(title)} - Wikipedia\n---\n{txt}\n")
            sys.stderr.write(f"[web] reponse: {title}\n")
            return

    results = google_search(query)
    if results:
        lines = [f"q={ascii_only(query)}"]
        for title, link, snippet in results:
            dom = re.sub(r"^https?://(www\.)?", "", link).split("/")[0]
            lines.append(f"{ascii_only(title)[:110]}|{link}|{ascii_only(dom)}|{ascii_only(snippet)[:200]}")
        write(RESULTS, "\n".join(lines) + "\n")
        write(ANS, "")
        sys.stderr.write(f"[web] {len(results)} resultats: {query}\n")
        return

    # ni Wikipédia ni Google (pas de clé) -> navigateur direct, comme avant
    url = ("https://www.google.com/search?q=" + query.replace(" ", "+"))
    open_in_firefox(url)
    write(ANS, f"q={ascii_only(query)}\nopen\n---\n{url}\n")
    sys.stderr.write(f"[web] navigateur: {url}\n")


# --- boucle -------------------------------------------------------------
for f in (ANS, RESULTS, ARTICLE):
    write(f, "")
try:
    open(REQ, "w").close()
    open(OPEN_REQ, "w").close()
    open(FIREFOX_REQ, "w").close()
except Exception:
    pass

last_req, last_open, last_ff = "", "", ""
sys.stderr.write(f"[web] surveille {REQ}\n")
while True:
    try:
        cur = open(REQ, encoding="utf-8").read()
    except Exception:
        cur = ""
    if cur and cur != last_req:
        last_req = cur
        lines = cur.splitlines()
        if len(lines) >= 2:
            try:
                handle_search(lines[1])
            except Exception as e:
                sys.stderr.write(f"[web] erreur recherche: {e}\n")

    try:
        cur2 = open(OPEN_REQ, encoding="utf-8").read()
    except Exception:
        cur2 = ""
    if cur2 and cur2 != last_open:
        last_open = cur2
        lines = cur2.splitlines()
        if len(lines) >= 2:
            try:
                handle_open(lines[1])
            except Exception as e:
                sys.stderr.write(f"[web] erreur ouverture: {e}\n")

    try:
        cur3 = open(FIREFOX_REQ, encoding="utf-8").read().strip()
    except Exception:
        cur3 = ""
    if cur3 and cur3 != last_ff:
        last_ff = cur3
        open_in_firefox(cur3)
        sys.stderr.write(f"[web] Firefox force: {cur3}\n")

    time.sleep(0.4)
