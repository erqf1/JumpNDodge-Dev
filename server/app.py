"""JumpNDodge Online-Leaderboard-Server.

Minimaler Flask-Service: Profil-Snapshot + Highscore pro (Name, Modus,
Schwierigkeit) speichern, Ranglisten/Suche/öffentliches Profil auslesen,
Verified-Keys einlösen, Namen ändern.

Spieler werden intern über token_hash identifiziert, nicht über den Namen -
der Name ist nur ein austauschbares Anzeigefeld (siehe /api/rename). Der
Score selbst bleibt eine Client-Meldung (siehe Plan/README) - Token und
Plausibilitätsgrenzen verhindern Namens-Diebstahl und grob unsinnige Werte,
aber keinen Angreifer, der gezielt eigene Requests baut.
"""

import hashlib
import json
import time

from flask import Flask, jsonify, request

import codes
import db
from filter import is_name_allowed, load_banned_words

app = Flask(__name__)
db.init_db()

HARD_SCORE_CEILING = 1_000_000
RATE_LIMIT_SECONDS = 2
_last_submit = {}


def _hash_secret(value):
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _as_int_score(value):
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return None


def _is_verified(conn, token_hash):
    row = conn.execute(
        "SELECT 1 FROM verified_keys WHERE claimed_by_token_hash = ? AND status = 'claimed'",
        (token_hash,),
    ).fetchone()
    return row is not None


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

    token_hash = _hash_secret(token)

    now = time.time()
    if now - _last_submit.get(token_hash, 0) < RATE_LIMIT_SECONDS:
        return jsonify(ok=False, error="rate_limited"), 429

    conn = db.get_connection()
    try:
        existing = conn.execute(
            "SELECT name FROM players WHERE token_hash = ?", (token_hash,)
        ).fetchone()
        if existing is None:
            # Neue Identität - der Name muss frei sein und den Filter bestehen.
            name_row = conn.execute(
                "SELECT token_hash FROM players WHERE name = ?", (name,)
            ).fetchone()
            if name_row is not None:
                return jsonify(ok=False, error="name_taken"), 403
            if not is_name_allowed(name):
                return jsonify(ok=False, error="name_filtered"), 400
        else:
            # Name kommt von der Identität (Token), nicht vom Client - Umbenennen
            # läuft ausschliesslich über /api/rename, nicht nebenbei über submit.
            name = existing["name"]

        _last_submit[token_hash] = now

        conn.execute("""
            INSERT INTO players (token_hash, name, player_color, stats_json, owned_powers_json,
                                  equipped_powers_json, cosmetics_json, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))
            ON CONFLICT(token_hash) DO UPDATE SET
                player_color = excluded.player_color,
                stats_json = excluded.stats_json,
                owned_powers_json = excluded.owned_powers_json,
                equipped_powers_json = excluded.equipped_powers_json,
                cosmetics_json = excluded.cosmetics_json,
                updated_at = excluded.updated_at
        """, (
            token_hash, name, snapshot.get("player_color"),
            json.dumps(snapshot.get("stats") or {}),
            json.dumps(snapshot.get("owned_powers") or []),
            json.dumps(snapshot.get("equipped_powers") or []),
            json.dumps(snapshot.get("cosmetics") or {}),
        ))

        existing_score = conn.execute(
            "SELECT score FROM scores WHERE token_hash = ? AND mode = ? AND difficulty = ?",
            (token_hash, mode, difficulty),
        ).fetchone()
        is_new_record = existing_score is None or score > existing_score["score"]
        if is_new_record:
            conn.execute("""
                INSERT INTO scores (token_hash, mode, difficulty, score, powers_used_json, submitted_at)
                VALUES (?, ?, ?, ?, ?, datetime('now'))
                ON CONFLICT(token_hash, mode, difficulty) DO UPDATE SET
                    score = excluded.score,
                    powers_used_json = excluded.powers_used_json,
                    submitted_at = excluded.submitted_at
            """, (token_hash, mode, difficulty, score, json.dumps(powers_used)))
        conn.commit()
    finally:
        conn.close()

    return jsonify(ok=True, new_record=is_new_record, name=name)


@app.post("/api/redeem_key")
def redeem_key():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    key = (body.get("key") or "").strip()

    if not token or not key:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    key_hash = _hash_secret(key)

    conn = db.get_connection()
    try:
        player = conn.execute(
            "SELECT token_hash FROM players WHERE token_hash = ?", (token_hash,)
        ).fetchone()
        if player is None:
            return jsonify(ok=False, error="unknown_player"), 404

        key_row = conn.execute(
            "SELECT * FROM verified_keys WHERE key_hash = ?", (key_hash,)
        ).fetchone()
        if key_row is None:
            return jsonify(ok=False, error="invalid_key"), 404

        # Key wird (neu) zugewiesen - egal ob vorher unbenutzt, von dir selbst
        # oder von jemand anderem gehalten. Wird er weitergegeben, "erbt" der
        # neue Einlöser einfach den Verified-Status; der alte Halter verliert
        # ihn automatisch, weil verified davon abgeleitet wird, ob DIESER
        # Token gerade als claimed_by_token_hash für diesen Key eingetragen
        # ist - kein separates "gesperrt"-Signal nötig, der Key bleibt nutzbar.
        conn.execute(
            "UPDATE verified_keys SET status='claimed', claimed_by_token_hash=?, claimed_at=datetime('now') "
            "WHERE key_hash=?",
            (token_hash, key_hash),
        )
        conn.commit()
        return jsonify(ok=True, verified=True)
    finally:
        conn.close()


@app.post("/api/redeem_code")
def redeem_code():
    """Allgemeiner Promo-Code (z.B. Gewinnspiel) gegen Coins - unabhängig von
    den Verified-Keys. Coins werden NICHT serverseitig gutgeschrieben (der
    Server ist keine Coin-Datenbank, das bleibt wie der Rest der Progression
    client-seitig) - die Antwort sagt dem Client nur, wie viele Coins er sich
    selbst gutschreiben darf, und merkt sich, dass dieser Spieler diesen Code
    schon benutzt hat."""
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    code = (body.get("code") or "").strip().upper()

    if not token or not code:
        return jsonify(ok=False, error="bad_request"), 400

    available = codes.load_codes()
    if code not in available:
        return jsonify(ok=False, error="invalid_code"), 404
    coins = available[code]

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        player = conn.execute(
            "SELECT token_hash FROM players WHERE token_hash = ?", (token_hash,)
        ).fetchone()
        if player is None:
            return jsonify(ok=False, error="unknown_player"), 404

        already = conn.execute(
            "SELECT 1 FROM redeemed_codes WHERE token_hash = ? AND code = ?",
            (token_hash, code),
        ).fetchone()
        if already is not None:
            return jsonify(ok=False, error="already_redeemed"), 403

        conn.execute(
            "INSERT INTO redeemed_codes (token_hash, code, redeemed_at) VALUES (?, ?, datetime('now'))",
            (token_hash, code),
        )
        conn.commit()
        return jsonify(ok=True, coins=coins)
    finally:
        conn.close()


@app.post("/api/rename")
def rename_player():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    new_name = (body.get("new_name") or "").strip()

    if not token or not new_name:
        return jsonify(ok=False, error="bad_request"), 400
    if not is_name_allowed(new_name):
        return jsonify(ok=False, error="name_filtered"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        player = conn.execute(
            "SELECT name FROM players WHERE token_hash = ?", (token_hash,)
        ).fetchone()
        if player is None:
            return jsonify(ok=False, error="unknown_player"), 404
        if player["name"] == new_name:
            return jsonify(ok=True, name=new_name)

        clash = conn.execute(
            "SELECT token_hash FROM players WHERE name = ?", (new_name,)
        ).fetchone()
        if clash is not None:
            return jsonify(ok=False, error="name_taken"), 403

        conn.execute(
            "UPDATE players SET name = ?, updated_at = datetime('now') WHERE token_hash = ?",
            (new_name, token_hash),
        )
        conn.commit()
        return jsonify(ok=True, name=new_name)
    finally:
        conn.close()


@app.post("/api/my_rank")
def my_rank():
    """Eigener Rang in einem (Modus, Schwierigkeit)-Bucket, auch ausserhalb der
    Top-N aus /api/leaderboard - fürs "Dock" am unteren Bildschirmrand. POST
    statt GET, damit der Token nicht im Query-String landet (der taucht sonst
    im Klartext in den Server-Logs auf, siehe access log)."""
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    mode = (body.get("mode") or "").strip()
    difficulty = (body.get("difficulty") or "").strip()
    if not token or not mode or not difficulty:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        mine = conn.execute(
            "SELECT score, submitted_at FROM scores WHERE token_hash = ? AND mode = ? AND difficulty = ?",
            (token_hash, mode, difficulty),
        ).fetchone()
        if mine is None:
            return jsonify(ok=True, rank=None)

        ahead = conn.execute("""
            SELECT COUNT(*) AS c FROM scores
            WHERE mode = ? AND difficulty = ?
              AND (score > ? OR (score = ? AND submitted_at < ?))
        """, (mode, difficulty, mine["score"], mine["score"], mine["submitted_at"])).fetchone()

        return jsonify(ok=True, rank=ahead["c"] + 1, score=mine["score"])
    finally:
        conn.close()


@app.get("/api/leaderboard")
def leaderboard():
    """?names=Name1,Name2,... schränkt optional auf eine bestimmte Spielerliste
    ein (z.B. fürs Freunde-Filter) - Reihenfolge/Response-Form bleibt gleich,
    nur eben nicht die globale Top-Liste sondern nur diese Namen."""
    mode = request.args.get("mode", "")
    difficulty = request.args.get("difficulty", "")
    names_param = request.args.get("names", "").strip()
    try:
        limit = min(max(int(request.args.get("limit", 50)), 1), 200)
    except ValueError:
        limit = 50

    conn = db.get_connection()
    try:
        base_query = """
            SELECT p.name AS name, s.score AS score, s.powers_used_json AS powers_used_json,
                   s.submitted_at AS submitted_at,
                   EXISTS(
                       SELECT 1 FROM verified_keys vk
                       WHERE vk.claimed_by_token_hash = s.token_hash AND vk.status = 'claimed'
                   ) AS verified
            FROM scores s JOIN players p ON p.token_hash = s.token_hash
            WHERE s.mode = ? AND s.difficulty = ?
        """
        params = [mode, difficulty]
        if names_param:
            names = [n.strip() for n in names_param.split(",") if n.strip()][:100]
            if not names:
                return jsonify(ok=True, entries=[])
            base_query += f" AND p.name IN ({','.join('?' for _ in names)})"
            params.extend(names)
        base_query += " ORDER BY s.score DESC, s.submitted_at ASC LIMIT ?"
        params.append(limit)
        rows = conn.execute(base_query, params).fetchall()
    finally:
        conn.close()

    return jsonify(ok=True, entries=[
        {
            "rank": i + 1,
            "name": r["name"],
            "score": r["score"],
            "powers_used": json.loads(r["powers_used_json"] or "[]"),
            "submitted_at": r["submitted_at"],
            "verified": bool(r["verified"]),
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
        rows = conn.execute("""
            SELECT name,
                   EXISTS(
                       SELECT 1 FROM verified_keys vk
                       WHERE vk.claimed_by_token_hash = players.token_hash AND vk.status = 'claimed'
                   ) AS verified
            FROM players WHERE name LIKE ? ORDER BY name LIMIT 25
        """, (f"%{q}%",)).fetchall()
    finally:
        conn.close()
    return jsonify(ok=True, results=[
        {"name": r["name"], "verified": bool(r["verified"])} for r in rows
    ])


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
            "SELECT mode, difficulty, score, powers_used_json FROM scores WHERE token_hash = ?",
            (player["token_hash"],),
        ).fetchall()
        verified = _is_verified(conn, player["token_hash"])
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
        "verified": verified,
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
