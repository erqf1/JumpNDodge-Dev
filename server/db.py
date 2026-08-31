"""SQLite-Setup und Verbindungs-Helfer für das JumpNDodge-Online-Leaderboard.

Spieler werden intern über token_hash identifiziert (nicht über den Namen),
damit ein Namenswechsel (siehe /api/rename in app.py) nichts an Scores/
Verified-Status ändern muss - der Name ist nur ein austauschbares Anzeigefeld.
"""

import hashlib
import os
import secrets
import sqlite3
import string

DATA_DIR = os.path.join(os.path.dirname(__file__), "data")
DB_PATH = os.path.join(DATA_DIR, "leaderboard.db")

VERIFIED_KEY_COUNT = 25
VERIFIED_KEY_LENGTH = 99
_KEY_ALPHABET = string.ascii_letters + string.digits


def get_connection():
    os.makedirs(DATA_DIR, exist_ok=True)
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    return conn


def _generate_key(length=VERIFIED_KEY_LENGTH):
    return "".join(secrets.choice(_KEY_ALPHABET) for _ in range(length))


def _seed_verified_keys_if_empty(conn):
    """Erzeugt beim allerersten Start VERIFIED_KEY_COUNT zufällige Keys und
    druckt sie im Klartext in die Container-Logs (einsehbar z.B. über
    Portainer) - das ist die einzige Stelle, an der die Klartext-Keys je
    existieren. Danach steht nur noch der Hash in der DB."""
    existing = conn.execute("SELECT COUNT(*) AS c FROM verified_keys").fetchone()["c"]
    if existing > 0:
        return
    print("=" * 70, flush=True)
    print(f"[SETUP] {VERIFIED_KEY_COUNT} neue Verified-Keys erzeugt (je {VERIFIED_KEY_LENGTH} Zeichen).", flush=True)
    print("Diese Liste wird NIRGENDS sonst gespeichert - jetzt sichern!", flush=True)
    for i in range(VERIFIED_KEY_COUNT):
        key = _generate_key()
        key_hash = hashlib.sha256(key.encode("utf-8")).hexdigest()
        conn.execute(
            "INSERT INTO verified_keys (key_hash, status) VALUES (?, 'unused')",
            (key_hash,),
        )
        print(f"  {i + 1:2d}. {key}", flush=True)
    conn.commit()
    print("=" * 70, flush=True)


def init_db():
    conn = get_connection()
    try:
        conn.execute("""
            CREATE TABLE IF NOT EXISTS players (
                token_hash TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                player_color TEXT,
                stats_json TEXT,
                owned_powers_json TEXT,
                equipped_powers_json TEXT,
                cosmetics_json TEXT,
                updated_at TEXT
            )
        """)
        conn.execute("""
            CREATE TABLE IF NOT EXISTS scores (
                token_hash TEXT NOT NULL,
                mode TEXT NOT NULL,
                difficulty TEXT NOT NULL,
                score INTEGER NOT NULL,
                powers_used_json TEXT,
                submitted_at TEXT,
                PRIMARY KEY (token_hash, mode, difficulty),
                FOREIGN KEY (token_hash) REFERENCES players(token_hash) ON DELETE CASCADE
            )
        """)
        conn.execute("""
            CREATE TABLE IF NOT EXISTS verified_keys (
                key_hash TEXT PRIMARY KEY,
                claimed_by_token_hash TEXT,
                status TEXT NOT NULL DEFAULT 'unused',
                claimed_at TEXT
            )
        """)
        conn.execute("""
            CREATE TABLE IF NOT EXISTS redeemed_codes (
                token_hash TEXT NOT NULL,
                code TEXT NOT NULL,
                redeemed_at TEXT,
                PRIMARY KEY (token_hash, code)
            )
        """)
        conn.commit()
        _seed_verified_keys_if_empty(conn)
    finally:
        conn.close()
