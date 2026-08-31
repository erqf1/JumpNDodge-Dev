"""Allgemeine Redeem-Codes (z.B. für Gewinnspiele/Promo-Aktionen) - Coins pro
Code. Liegt als Textdatei im Daten-Volume (nicht im Image!), damit der
Server-Betreiber jederzeit neue Codes ergänzen kann, ohne den Container neu
zu bauen/deployen - einfach über den CasaOS-Dateibrowser editieren.

Format (eine Zeile pro Code, Kommentare mit #):
    CODE:COINS
    LAUNCH2026:500
"""

import os

CODES_PATH = os.path.join(os.path.dirname(__file__), "data", "redeem_codes.txt")

_cache = None
_cache_mtime = None


def load_codes():
    """Lädt die Code-Liste mit einfachem mtime-Cache (Datei kann sich zur
    Laufzeit ändern, wenn der Betreiber sie direkt editiert). Gibt ein Dict
    CODE -> Coins zurück; leeres Dict falls die Datei (noch) nicht existiert."""
    global _cache, _cache_mtime
    try:
        mtime = os.path.getmtime(CODES_PATH)
    except OSError:
        if not os.path.isfile(CODES_PATH):
            os.makedirs(os.path.dirname(CODES_PATH), exist_ok=True)
            with open(CODES_PATH, "w", encoding="utf-8") as f:
                f.write("# Ein Code pro Zeile im Format CODE:COINS, z.B.\n# LAUNCH2026:500\n")
        return {}
    if _cache is not None and mtime == _cache_mtime:
        return _cache

    codes = {}
    with open(CODES_PATH, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#") or ":" not in line:
                continue
            code, _, coins = line.partition(":")
            code = code.strip().upper()
            try:
                coins = int(coins.strip())
            except ValueError:
                continue
            if code:
                codes[code] = coins
    _cache = codes
    _cache_mtime = mtime
    return codes
