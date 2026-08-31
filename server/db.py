"""SQLite-Setup und Verbindungs-Helfer für das JumpNDodge-Online-Leaderboard."""

import os
import sqlite3

DATA_DIR = os.path.join(os.path.dirname(__file__), "data")
DB_PATH = os.path.join(DATA_DIR, "leaderboard.db")


def get_connection():
    os.makedirs(DATA_DIR, exist_ok=True)
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    return conn


def init_db():
    conn = get_connection()
    try:
        conn.execute("""
            CREATE TABLE IF NOT EXISTS players (
                name TEXT PRIMARY KEY,
                token_hash TEXT NOT NULL,
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
                name TEXT NOT NULL,
                mode TEXT NOT NULL,
                difficulty TEXT NOT NULL,
                score INTEGER NOT NULL,
                powers_used_json TEXT,
                submitted_at TEXT,
                PRIMARY KEY (name, mode, difficulty),
                FOREIGN KEY (name) REFERENCES players(name) ON DELETE CASCADE
            )
        """)
        conn.commit()
    finally:
        conn.close()
