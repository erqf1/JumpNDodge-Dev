"""JumpNDodge Online-Leaderboard-Server.

Minimaler Flask-Service: Profil-Snapshot + Highscore pro (Name, Modus,
Schwierigkeit) speichern, Ranglisten/Suche/öffentliches Profil auslesen.
Der Score selbst bleibt eine Client-Meldung (siehe Plan/README) - Token und
Plausibilitätsgrenzen verhindern Namens-Diebstahl und grob unsinnige Werte,
aber keinen Angreifer, der gezielt eigene Requests baut.
"""

import hashlib
import json
import time

from flask import Flask, jsonify, request

import db
from filter import is_name_allowed, load_banned_words

app = Flask(__name__)
db.init_db()

HARD_SCORE_CEILING = 1_000_000
RATE_LIMIT_SECONDS = 2
_last_submit = {}


def _hash_token(token):
    return hashlib.sha256(token.encode("utf-8")).hexdigest()


def _as_int_score(value):
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return None


@app.post("/api/submit")
def submit_score():
    body = request.get_json(silent=True) or {}
    name = (body.get("name") or "").strip()
    token = body.get("token") or ""
    mode = (body.get("mode") or "").strip()
    difficulty = (body.get("difficulty") or "").strip()
    score = _as_int_score(body.get("score"))
    powers_used = body.get("powers_used") or []
    snapshot = body.get("profile_snapshot") or {}

    if not name or not token or not mode or not difficulty or score is None:
        return jsonify(ok=False, error="bad_request"), 400
    if score < 0 or score > HARD_SCORE_CEILING:
        return jsonify(ok=False, error="score_out_of_range"), 400
    if not isinstance(powers_used, list):
        return jsonify(ok=False, error="bad_request"), 400

    now = time.time()
    if now - _last_submit.get(name, 0) < RATE_LIMIT_SECONDS:
        return jsonify(ok=False, error="rate_limited"), 429

    token_hash = _hash_token(token)
    conn = db.get_connection()
    try:
        row = conn.execute(
            "SELECT token_hash FROM players WHERE name = ?", (name,)
        ).fetchone()
        if row is None:
            if not is_name_allowed(name):
                return jsonify(ok=False, error="name_filtered"), 400
        elif row["token_hash"] != token_hash:
            return jsonify(ok=False, error="name_taken"), 403

        _last_submit[name] = now

        conn.execute("""
            INSERT INTO players (name, token_hash, player_color, stats_json, owned_powers_json,
                                  equipped_powers_json, cosmetics_json, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))
            ON CONFLICT(name) DO UPDATE SET
                token_hash = excluded.token_hash,
                player_color = excluded.player_color,
                stats_json = excluded.stats_json,
                owned_powers_json = excluded.owned_powers_json,
                equipped_powers_json = excluded.equipped_powers_json,
                cosmetics_json = excluded.cosmetics_json,
                updated_at = excluded.updated_at
        """, (
            name, token_hash, snapshot.get("player_color"),
            json.dumps(snapshot.get("stats") or {}),
            json.dumps(snapshot.get("owned_powers") or []),
            json.dumps(snapshot.get("equipped_powers") or []),
            json.dumps(snapshot.get("cosmetics") or {}),
        ))

        existing = conn.execute(
            "SELECT score FROM scores WHERE name = ? AND mode = ? AND difficulty = ?",
            (name, mode, difficulty),
        ).fetchone()
        is_new_record = existing is None or score > existing["score"]
        if is_new_record:
            conn.execute("""
                INSERT INTO scores (name, mode, difficulty, score, powers_used_json, submitted_at)
                VALUES (?, ?, ?, ?, ?, datetime('now'))
                ON CONFLICT(name, mode, difficulty) DO UPDATE SET
                    score = excluded.score,
                    powers_used_json = excluded.powers_used_json,
                    submitted_at = excluded.submitted_at
            """, (name, mode, difficulty, score, json.dumps(powers_used)))
        conn.commit()
    finally:
        conn.close()

    return jsonify(ok=True, new_record=is_new_record)


@app.get("/api/leaderboard")
def leaderboard():
    mode = request.args.get("mode", "")
    difficulty = request.args.get("difficulty", "")
    try:
        limit = min(max(int(request.args.get("limit", 50)), 1), 200)
    except ValueError:
        limit = 50

    conn = db.get_connection()
    try:
        rows = conn.execute("""
            SELECT name, score, powers_used_json, submitted_at FROM scores
            WHERE mode = ? AND difficulty = ?
            ORDER BY score DESC, submitted_at ASC
            LIMIT ?
        """, (mode, difficulty, limit)).fetchall()
    finally:
        conn.close()

    return jsonify(ok=True, entries=[
        {
            "rank": i + 1,
            "name": r["name"],
            "score": r["score"],
            "powers_used": json.loads(r["powers_used_json"] or "[]"),
            "submitted_at": r["submitted_at"],
        }
        for i, r in enumerate(rows)
    ])


@app.get("/api/players/search")
def search_players():
    q = request.args.get("q", "").strip()
    if not q:
        return jsonify(ok=True, results=[])
    conn = db.get_connection()
    try:
        rows = conn.execute(
            "SELECT name FROM players WHERE name LIKE ? ORDER BY name LIMIT 25",
            (f"%{q}%",),
        ).fetchall()
    finally:
        conn.close()
    return jsonify(ok=True, results=[r["name"] for r in rows])


@app.get("/api/players/<name>")
def get_player(name):
    conn = db.get_connection()
    try:
        player = conn.execute(
            "SELECT * FROM players WHERE name = ?", (name,)
        ).fetchone()
        if player is None:
            return jsonify(ok=False, error="not_found"), 404
        scores = conn.execute(
            "SELECT mode, difficulty, score, powers_used_json FROM scores WHERE name = ?",
            (name,),
        ).fetchall()
    finally:
        conn.close()

    return jsonify(ok=True, player={
        "name": player["name"],
        "player_color": player["player_color"],
        "stats": json.loads(player["stats_json"] or "{}"),
        "owned_powers": json.loads(player["owned_powers_json"] or "[]"),
        "equipped_powers": json.loads(player["equipped_powers_json"] or "[]"),
        "cosmetics": json.loads(player["cosmetics_json"] or "{}"),
        "updated_at": player["updated_at"],
        "scores": [
            {
                "mode": s["mode"],
                "difficulty": s["difficulty"],
                "score": s["score"],
                "powers_used": json.loads(s["powers_used_json"] or "[]"),
            }
            for s in scores
        ],
    })


@app.get("/api/banned_words")
def banned_words():
    return jsonify(ok=True, words=load_banned_words())


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000, threaded=True)
