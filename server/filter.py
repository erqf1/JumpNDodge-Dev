"""Namensfilter für Profilnamen (Slurs/Beleidigungen).

Bewusst simpel gehalten: Normalisierung (Gross-/Kleinschreibung, Akzente,
einfaches Leetspeak) + Substring-Abgleich gegen eine Textdatei. Die Blockliste
ist nur ein kleiner Startsatz - direkt auf dem Server in banned_words.txt
erweiterbar, ohne dass Server oder Client neu gebaut werden müssen.
"""

import os
import unicodedata

BANNED_WORDS_PATH = os.path.join(os.path.dirname(__file__), "banned_words.txt")

_LEET_MAP = str.maketrans({
    "0": "o", "1": "i", "3": "e", "4": "a", "5": "s", "7": "t",
    "@": "a", "$": "s",
})

_words_cache = None
_words_cache_mtime = None


def _normalize(text):
    text = text.casefold()
    text = unicodedata.normalize("NFKD", text)
    text = "".join(c for c in text if not unicodedata.combining(c))
    text = text.translate(_LEET_MAP)
    text = "".join(c for c in text if c.isalnum())
    return text


def load_banned_words():
    """Lädt die Blockliste, mit einfachem mtime-Cache (Datei kann sich zur
    Laufzeit ändern, wenn der Server-Betreiber sie direkt editiert)."""
    global _words_cache, _words_cache_mtime
    try:
        mtime = os.path.getmtime(BANNED_WORDS_PATH)
    except OSError:
        return []
    if _words_cache is not None and mtime == _words_cache_mtime:
        return _words_cache
    with open(BANNED_WORDS_PATH, encoding="utf-8") as f:
        words = [line.strip() for line in f if line.strip() and not line.startswith("#")]
    _words_cache = words
    _words_cache_mtime = mtime
    return words


def _contains_banned_word(normalized_text):
    for word in load_banned_words():
        needle = _normalize(word)
        if needle and needle in normalized_text:
            return True
    return False


def is_name_allowed(name):
    if not name or not name.strip():
        return False
    normalized = _normalize(name)
    if not normalized:
        return False
    return not _contains_banned_word(normalized)


def is_text_allowed(text):
    """Wie is_name_allowed(), aber für frei formulierten Text (Chat-Nachrichten):
    anders als ein Name darf Text auch mal nur aus Satzzeichen/Leerzeichen bestehen
    (z.B. "..." oder "!!!") - das ist nichts, was gegen die Blockliste zu prüfen
    wäre, also erlaubt statt wie bei is_name_allowed() abgelehnt."""
    if not text or not text.strip():
        return False
    normalized = _normalize(text)
    if not normalized:
        return True
    return not _contains_banned_word(normalized)
