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
import re
import time

from flask import Flask, jsonify, request

import codes
import db
from filter import is_name_allowed, is_text_allowed, load_banned_words

app = Flask(__name__)
db.init_db()

# Muss mit einem Buchstaben anfangen (vermeidet Verwechslung mit rein
# numerischen IDs/Zahlen-Spam), 3-20 Zeichen, nur ASCII-Alnum + Unterstrich -
# Eindeutigkeit wird case-insensitiv geprüft (siehe idx_players_username in
# db.py), Gross-/Kleinschreibung bleibt aber wie eingegeben gespeichert.
_USERNAME_RE = re.compile(r"^[A-Za-z][A-Za-z0-9_]{2,19}$")

HARD_SCORE_CEILING = 1_000_000
RATE_LIMIT_SECONDS = 2
_last_submit = {}

_rate_limit_last = {}


def _rate_limited(bucket, token_hash, seconds):
    """Einfacher In-Memory Rate-Limiter pro (bucket, token) - gleiche Idee wie
    _last_submit oben bei submit_score(), nur als wiederverwendbarer Helfer für
    neue Endpunkte (Freundschaftsanfragen, Chat) statt das Muster zu
    duplizieren. submit_score() selbst bleibt bewusst unverändert bei seinem
    eigenen _last_submit, um an diesem bereits produktiv laufenden Code nichts
    ohne Not zu verändern."""
    key = (bucket, token_hash)
    now = time.time()
    if now - _rate_limit_last.get(key, 0) < seconds:
        return True
    _rate_limit_last[key] = now
    return False


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


def _get_player_by_name(conn, name):
    return conn.execute("SELECT token_hash, name FROM players WHERE name = ?", (name,)).fetchone()


def _get_player_by_username(conn, username):
    return conn.execute(
        "SELECT token_hash, name, username FROM players WHERE username = ? COLLATE NOCASE", (username,)
    ).fetchone()


def _get_player_by_token(conn, token_hash):
    return conn.execute(
        "SELECT token_hash, name, username FROM players WHERE token_hash = ?", (token_hash,)
    ).fetchone()


def _get_clan_tag_for(conn, token_hash):
    row = conn.execute("""
        SELECT c.tag AS tag FROM clan_members cm JOIN clans c ON c.clan_id = cm.clan_id
        WHERE cm.token_hash = ?
    """, (token_hash,)).fetchone()
    return row["tag"] if row else None


def _resolve_unique_name(conn, base_name):
    """Gibt base_name zurück, falls frei - sonst den kleinsten freien
    "base_name#N" (N ab 2). players.name ist UNIQUE (siehe db.py), früher
    wurde eine Kollision deshalb hart mit name_taken abgewiesen - das führte
    dazu, dass zwei Spieler mit demselben gewünschten Namen (z.B. beide
    "Alex") JEDEN Submit für immer stillschweigend scheitern sahen, ohne
    Fehlermeldung (Client hat bei /api/submit bisher gar kein on_done, siehe
    JumpNDodge.py submit_score_online()). Der erstregistrierte Name bleibt
    unverändert (kein rückwirkendes Umhängen auf "#1")."""
    row = conn.execute("SELECT 1 FROM players WHERE name = ?", (base_name,)).fetchone()
    if row is None:
        return base_name
    used = conn.execute(
        "SELECT name FROM players WHERE name = ? OR name LIKE ? ESCAPE '\\'",
        (base_name, base_name.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_") + "#%"),
    ).fetchall()
    taken_numbers = {1}
    for r in used:
        suffix = r["name"][len(base_name):]
        if suffix.startswith("#") and suffix[1:].isdigit():
            taken_numbers.add(int(suffix[1:]))
    n = 2
    while n in taken_numbers:
        n += 1
    return f"{base_name}#{n}"


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
            # Neue Identität - der Name muss den Filter bestehen; eine Kollision mit
            # einem bereits vergebenen Namen wird nicht abgewiesen, sondern durch
            # _resolve_unique_name() automatisch zu "Name#2"/"Name#3"/... aufgelöst.
            if not is_name_allowed(name):
                return jsonify(ok=False, error="name_filtered"), 400
            name = _resolve_unique_name(conn, name)
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


@app.post("/api/delete_account")
def delete_account():
    """Löscht einen Spieler vollständig (Account-Löschung, wenn das lokale
    Profil gelöscht wird - siehe JumpNDodge.py delete_profile()). scores hängt
    an ON DELETE CASCADE (db.py), wird also automatisch mitgelöscht. Ein
    claimed Verified-Key wird freigegeben statt an einem gelöschten Account
    hängen zu bleiben - sonst dauerhaft für niemanden mehr einlösbar, das
    widerspricht dem Transfer-Modell aus /api/redeem_key. Kein Player mit
    diesem Token (nie online registriert, oder schon gelöscht) zählt als
    Erfolg - das Ziel "online existiert nichts mehr unter diesem Token" ist
    dann ja bereits erreicht, wichtig für den retry-sicheren Client-seitigen
    Offline-Flush (siehe _flush_pending_online_deletions())."""
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    if not token:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        player = conn.execute(
            "SELECT token_hash FROM players WHERE token_hash = ?", (token_hash,)
        ).fetchone()
        if player is None:
            return jsonify(ok=True)

        conn.execute(
            "UPDATE verified_keys SET status='unused', claimed_by_token_hash=NULL, claimed_at=NULL "
            "WHERE claimed_by_token_hash = ?",
            (token_hash,),
        )
        # Gleicher Owner-Nachfolge-Pfad wie /api/clans/leave - ein gelöschter
        # Account darf keinen Clan besitzend zurücklassen. friend_requests
        # räumt sich von selbst über ON DELETE CASCADE auf (PRAGMA foreign_keys
        # ist pro Connection an, siehe db.py get_connection()).
        _leave_clan_if_member(conn, token_hash)
        conn.execute("DELETE FROM players WHERE token_hash = ?", (token_hash,))
        conn.commit()
        return jsonify(ok=True)
    finally:
        conn.close()


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

        resolved_name = _resolve_unique_name(conn, new_name)
        conn.execute(
            "UPDATE players SET name = ?, updated_at = datetime('now') WHERE token_hash = ?",
            (resolved_name, token_hash),
        )
        conn.commit()
        return jsonify(ok=True, name=resolved_name)
    finally:
        conn.close()


@app.post("/api/set_username")
def set_username():
    """Setzt den stabilen, EINMALIGEN Username eines Spielers - anders als der
    Anzeigename (siehe /api/rename) niemals änderbar, damit Freundschafts-
    anfragen/Chat-Ziele/Kick-Ziele nie ins Leere laufen, wenn sich jemand
    umbenennt (der Anzeigename bleibt dafür komplett unberührt, weiterhin frei
    änderbar). Kann - anders als jeder andere Endpunkt hier ausser /api/submit
    - auf einen Token treffen, der noch gar keine players-Zeile hat: ein
    brandneues Profil hat unter Umständen noch nie einen Score eingereicht,
    bevor es hier zum ersten Mal online Kontakt aufnimmt (siehe Zwangs-
    Onboarding-Gate im Client) - deshalb wird in diesem Fall die Zeile hier
    gleich mit angelegt, analog zu submit_score()s Upsert."""
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    name = (body.get("name") or "").strip()
    username = (body.get("username") or "").strip()

    if not token or not username:
        return jsonify(ok=False, error="bad_request"), 400
    if not _USERNAME_RE.match(username):
        return jsonify(ok=False, error="invalid_format"), 400
    if not is_name_allowed(username):
        return jsonify(ok=False, error="name_filtered"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        existing = conn.execute(
            "SELECT username FROM players WHERE token_hash = ?", (token_hash,)
        ).fetchone()
        if existing is not None and existing["username"] is not None:
            return jsonify(ok=False, error="username_already_set"), 403

        taken = conn.execute(
            "SELECT 1 FROM players WHERE username = ? COLLATE NOCASE", (username,)
        ).fetchone()
        if taken is not None:
            return jsonify(ok=False, error="username_taken"), 403

        if existing is None:
            if not name or not is_name_allowed(name):
                return jsonify(ok=False, error="bad_request"), 400
            resolved_name = _resolve_unique_name(conn, name)
            conn.execute(
                "INSERT INTO players (token_hash, name, username, updated_at) VALUES (?, ?, ?, datetime('now'))",
                (token_hash, resolved_name, username),
            )
        else:
            resolved_name = None
            conn.execute(
                "UPDATE players SET username = ?, updated_at = datetime('now') WHERE token_hash = ?",
                (username, token_hash),
            )
        conn.commit()
        return jsonify(ok=True, username=username, name=resolved_name)
    finally:
        conn.close()


@app.get("/api/ping")
def ping():
    """Reiner Erreichbarkeits-Check, keine DB-Anfrage - der Client nutzt das
    fürs Zwangs-Onboarding-Gate (siehe JumpNDodge.py), um vor dem Anzeigen
    des Username-Formulars einmal frisch zu prüfen, ob der Server gerade
    überhaupt erreichbar ist, statt sich auf einen ggf. veralteten
    Hintergrund-Status zu verlassen."""
    return jsonify(ok=True)


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
    """?usernames=user1,user2,... schränkt optional auf eine bestimmte
    Spielerliste ein (z.B. fürs Freunde-Filter) - Reihenfolge/Response-Form
    bleibt gleich, nur eben nicht die globale Top-Liste sondern nur diese
    Spieler. Filtert über username (stabil), nicht name (änderbar) - sonst
    verschwindet ein umbenannter Freund stillschweigend aus der gefilterten
    Liste, siehe Kommentar bei _resolve_conversation()/set_username()."""
    mode = request.args.get("mode", "")
    difficulty = request.args.get("difficulty", "")
    usernames_param = request.args.get("usernames", "").strip()
    try:
        limit = min(max(int(request.args.get("limit", 50)), 1), 200)
    except ValueError:
        limit = 50

    conn = db.get_connection()
    try:
        base_query = """
            SELECT p.name AS name, p.username AS username, s.score AS score,
                   s.powers_used_json AS powers_used_json, s.submitted_at AS submitted_at,
                   EXISTS(
                       SELECT 1 FROM verified_keys vk
                       WHERE vk.claimed_by_token_hash = s.token_hash AND vk.status = 'claimed'
                   ) AS verified,
                   (
                       SELECT c.tag FROM clan_members cm JOIN clans c ON c.clan_id = cm.clan_id
                       WHERE cm.token_hash = s.token_hash
                   ) AS clan_tag
            FROM scores s JOIN players p ON p.token_hash = s.token_hash
            WHERE s.mode = ? AND s.difficulty = ?
        """
        params = [mode, difficulty]
        if usernames_param:
            usernames = [n.strip() for n in usernames_param.split(",") if n.strip()][:100]
            if not usernames:
                return jsonify(ok=True, entries=[])
            base_query += f" AND p.username COLLATE NOCASE IN ({','.join('?' for _ in usernames)})"
            params.extend(usernames)
        base_query += " ORDER BY s.score DESC, s.submitted_at ASC LIMIT ?"
        params.append(limit)
        rows = conn.execute(base_query, params).fetchall()
    finally:
        conn.close()

    return jsonify(ok=True, entries=[
        {
            "rank": i + 1,
            "name": r["name"],
            "username": r["username"],
            "score": r["score"],
            "powers_used": json.loads(r["powers_used_json"] or "[]"),
            "submitted_at": r["submitted_at"],
            "verified": bool(r["verified"]),
            "clan_tag": r["clan_tag"],
        }
        for i, r in enumerate(rows)
    ])


@app.get("/api/players/search")
def search_players():
    """Nur Spieler mit gesetztem Username werden gefunden - wer noch keinen
    hat (alle Alt-Accounts vor diesem Feature, bis zu ihrem nächsten Online-
    Start, siehe Zwangs-Onboarding-Gate im Client), taucht bewusst nicht auf,
    statt als unanfreundbare/unanschreibbare Karteileiche in den Ergebnissen
    zu erscheinen."""
    q = request.args.get("q", "").strip()
    if not q:
        return jsonify(ok=True, results=[])
    conn = db.get_connection()
    try:
        rows = conn.execute("""
            SELECT name, username,
                   EXISTS(
                       SELECT 1 FROM verified_keys vk
                       WHERE vk.claimed_by_token_hash = players.token_hash AND vk.status = 'claimed'
                   ) AS verified,
                   (
                       SELECT c.tag FROM clan_members cm JOIN clans c ON c.clan_id = cm.clan_id
                       WHERE cm.token_hash = players.token_hash
                   ) AS clan_tag
            FROM players WHERE username IS NOT NULL AND name LIKE ? ORDER BY name LIMIT 25
        """, (f"%{q}%",)).fetchall()
    finally:
        conn.close()
    return jsonify(ok=True, results=[
        {"name": r["name"], "username": r["username"], "verified": bool(r["verified"]), "clan_tag": r["clan_tag"]}
        for r in rows
    ])


@app.get("/api/players/<username>")
def get_player(username):
    conn = db.get_connection()
    try:
        player = conn.execute(
            "SELECT * FROM players WHERE username = ? COLLATE NOCASE", (username,)
        ).fetchone()
        if player is None:
            return jsonify(ok=False, error="not_found"), 404
        scores = conn.execute(
            "SELECT mode, difficulty, score, powers_used_json FROM scores WHERE token_hash = ?",
            (player["token_hash"],),
        ).fetchall()
        verified = _is_verified(conn, player["token_hash"])
        clan_tag = _get_clan_tag_for(conn, player["token_hash"])
    finally:
        conn.close()

    return jsonify(ok=True, player={
        "name": player["name"],
        "username": player["username"],
        "player_color": player["player_color"],
        "stats": json.loads(player["stats_json"] or "{}"),
        "owned_powers": json.loads(player["owned_powers_json"] or "[]"),
        "equipped_powers": json.loads(player["equipped_powers_json"] or "[]"),
        "cosmetics": json.loads(player["cosmetics_json"] or "{}"),
        "updated_at": player["updated_at"],
        "verified": verified,
        "clan_tag": clan_tag,
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


# =============================
# Freundschaftsanfragen
# =============================

@app.post("/api/friends/request")
def friend_request():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    to_username = (body.get("to_username") or "").strip()
    if not token or not to_username:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404
        target = _get_player_by_username(conn, to_username)
        if target is None:
            return jsonify(ok=False, error="target_not_found"), 404
        target_hash = target["token_hash"]
        if target_hash == token_hash:
            return jsonify(ok=False, error="bad_request"), 400

        existing_mine = conn.execute(
            "SELECT status FROM friend_requests WHERE from_token_hash = ? AND to_token_hash = ?",
            (token_hash, target_hash),
        ).fetchone()
        if existing_mine is not None and existing_mine["status"] == "accepted":
            return jsonify(ok=False, error="already_friends"), 403

        # Falls die Gegenseite bereits eine Anfrage an mich geschickt hat, wird
        # diese jetzt zu "accepted" befördert statt eine zweite, umgekehrte
        # Anfrage anzulegen (symmetrisches Auto-Accept) - so kennt der Client
        # nur zwei Button-Zustände (Add/Remove), keinen dritten "pending"
        # Zustand auf dem öffentlichen Profil.
        reverse = conn.execute(
            "SELECT status FROM friend_requests WHERE from_token_hash = ? AND to_token_hash = ?",
            (target_hash, token_hash),
        ).fetchone()
        if reverse is not None:
            if reverse["status"] == "accepted":
                return jsonify(ok=False, error="already_friends"), 403
            conn.execute(
                "UPDATE friend_requests SET status = 'accepted' WHERE from_token_hash = ? AND to_token_hash = ?",
                (target_hash, token_hash),
            )
            conn.commit()
            return jsonify(ok=True, status="accepted")

        conn.execute("""
            INSERT INTO friend_requests (from_token_hash, to_token_hash, status, created_at)
            VALUES (?, ?, 'pending', datetime('now'))
            ON CONFLICT(from_token_hash, to_token_hash) DO UPDATE SET
                status = 'pending', created_at = excluded.created_at
        """, (token_hash, target_hash))
        conn.commit()
        return jsonify(ok=True, status="pending")
    finally:
        conn.close()


@app.post("/api/friends/accept")
def friend_accept():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    from_username = (body.get("from_username") or "").strip()
    if not token or not from_username:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404
        other = _get_player_by_username(conn, from_username)
        if other is None:
            return jsonify(ok=False, error="target_not_found"), 404

        row = conn.execute(
            "SELECT status FROM friend_requests WHERE from_token_hash = ? AND to_token_hash = ?",
            (other["token_hash"], token_hash),
        ).fetchone()
        if row is None or row["status"] != "pending":
            return jsonify(ok=False, error="no_pending_request"), 404

        conn.execute(
            "UPDATE friend_requests SET status = 'accepted' WHERE from_token_hash = ? AND to_token_hash = ?",
            (other["token_hash"], token_hash),
        )
        conn.commit()
        return jsonify(ok=True)
    finally:
        conn.close()


@app.post("/api/friends/decline")
def friend_decline():
    """Löscht eine ausstehende Anfrage in beide Richtungen - deckt damit sowohl
    "eine an mich gerichtete Anfrage ablehnen" als auch "eine von mir gesendete
    Anfrage zurückziehen" mit demselben Endpunkt ab, ohne dass der Client
    wissen muss, wer ursprünglich angefragt hat."""
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    other_username = (body.get("other_username") or "").strip()
    if not token or not other_username:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404
        other = _get_player_by_username(conn, other_username)
        if other is None:
            return jsonify(ok=True)

        conn.execute(
            "DELETE FROM friend_requests WHERE status = 'pending' AND "
            "((from_token_hash = ? AND to_token_hash = ?) OR (from_token_hash = ? AND to_token_hash = ?))",
            (token_hash, other["token_hash"], other["token_hash"], token_hash),
        )
        conn.commit()
        return jsonify(ok=True)
    finally:
        conn.close()


@app.post("/api/friends/remove")
def friend_remove():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    other_username = (body.get("other_username") or "").strip()
    if not token or not other_username:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404
        other = _get_player_by_username(conn, other_username)
        if other is None:
            return jsonify(ok=True)

        conn.execute(
            "DELETE FROM friend_requests WHERE "
            "(from_token_hash = ? AND to_token_hash = ?) OR (from_token_hash = ? AND to_token_hash = ?)",
            (token_hash, other["token_hash"], other["token_hash"], token_hash),
        )
        conn.commit()
        return jsonify(ok=True)
    finally:
        conn.close()


@app.post("/api/friends/incoming")
def friend_incoming():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    if not token:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404

        rows = conn.execute("""
            SELECT p.name AS name, p.username AS username, fr.created_at AS created_at
            FROM friend_requests fr JOIN players p ON p.token_hash = fr.from_token_hash
            WHERE fr.to_token_hash = ? AND fr.status = 'pending'
            ORDER BY fr.created_at ASC
        """, (token_hash,)).fetchall()
        return jsonify(ok=True, requests=[
            {"from_name": r["name"], "from_username": r["username"], "created_at": r["created_at"]} for r in rows
        ])
    finally:
        conn.close()


@app.get("/api/friends/list")
def friend_list():
    username = request.args.get("username", "").strip()
    if not username:
        return jsonify(ok=False, error="bad_request"), 400

    conn = db.get_connection()
    try:
        player = _get_player_by_username(conn, username)
        if player is None:
            return jsonify(ok=False, error="not_found"), 404

        rows = conn.execute("""
            SELECT p.name AS name, p.username AS username FROM friend_requests fr
            JOIN players p ON p.token_hash = CASE
                WHEN fr.from_token_hash = ? THEN fr.to_token_hash ELSE fr.from_token_hash END
            WHERE fr.status = 'accepted' AND (fr.from_token_hash = ? OR fr.to_token_hash = ?)
            ORDER BY p.name COLLATE NOCASE
        """, (player["token_hash"], player["token_hash"], player["token_hash"])).fetchall()
        return jsonify(ok=True, friends=[
            {"name": r["name"], "username": r["username"]} for r in rows
        ])
    finally:
        conn.close()


# =============================
# Clans
# =============================

CLAN_MEMBER_CAP = 30


def _get_clan_by_tag(conn, tag):
    return conn.execute("SELECT * FROM clans WHERE tag = ?", (tag,)).fetchone()


def _get_member_row(conn, token_hash):
    return conn.execute("SELECT * FROM clan_members WHERE token_hash = ?", (token_hash,)).fetchone()


def _promote_or_disband(conn, clan_id):
    """Befördert das dienstälteste verbleibende Mitglied zum neuen Owner, oder
    löst den Clan komplett auf, falls niemand mehr übrig ist. Erwartet, dass
    der abtretende Owner schon aus clan_members entfernt wurde (siehe
    _leave_clan_if_member). Gibt True zurück, falls der Clan aufgelöst wurde."""
    next_owner = conn.execute(
        "SELECT token_hash FROM clan_members WHERE clan_id = ? ORDER BY joined_at ASC LIMIT 1",
        (clan_id,),
    ).fetchone()
    if next_owner is None:
        conn.execute("DELETE FROM clans WHERE clan_id = ?", (clan_id,))
        return True
    conn.execute(
        "UPDATE clans SET owner_token_hash = ? WHERE clan_id = ?",
        (next_owner["token_hash"], clan_id),
    )
    return False


def _leave_clan_if_member(conn, token_hash):
    """Entfernt token_hash aus seinem Clan (falls Mitglied) und befördert bei
    Bedarf einen neuen Owner / löst den Clan auf. Geteilte Logik zwischen
    /api/clans/leave und /api/delete_account, damit eine Account-Löschung
    denselben Owner-Nachfolge-Pfad durchläuft wie normales Verlassen - sonst
    bliebe ein gelöschter Account als Owner eines Clans stehen. Gibt True
    zurück, falls der Clan dabei aufgelöst wurde (auch False, falls gar keine
    Mitgliedschaft bestand)."""
    member = conn.execute("SELECT clan_id FROM clan_members WHERE token_hash = ?", (token_hash,)).fetchone()
    if member is None:
        return False
    clan_id = member["clan_id"]
    clan = conn.execute("SELECT owner_token_hash FROM clans WHERE clan_id = ?", (clan_id,)).fetchone()
    conn.execute("DELETE FROM clan_members WHERE clan_id = ? AND token_hash = ?", (clan_id, token_hash))
    if clan is not None and clan["owner_token_hash"] == token_hash:
        return _promote_or_disband(conn, clan_id)
    return False


@app.post("/api/clans/create")
def clan_create():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    tag = (body.get("tag") or "").strip().upper()
    name = (body.get("name") or "").strip()
    if not token or not tag or not name or len(tag) > 5 or len(name) > 24:
        return jsonify(ok=False, error="bad_request"), 400
    if not is_name_allowed(tag) or not is_name_allowed(name):
        return jsonify(ok=False, error="name_filtered"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404
        if _get_member_row(conn, token_hash) is not None:
            return jsonify(ok=False, error="already_in_clan"), 403
        if _get_clan_by_tag(conn, tag) is not None:
            return jsonify(ok=False, error="tag_taken"), 403
        if conn.execute("SELECT 1 FROM clans WHERE name = ?", (name,)).fetchone() is not None:
            return jsonify(ok=False, error="name_taken"), 403

        cur = conn.execute(
            "INSERT INTO clans (tag, name, owner_token_hash, created_at) VALUES (?, ?, ?, datetime('now'))",
            (tag, name, token_hash),
        )
        conn.execute(
            "INSERT INTO clan_members (clan_id, token_hash, joined_at) VALUES (?, ?, datetime('now'))",
            (cur.lastrowid, token_hash),
        )
        conn.commit()
        return jsonify(ok=True, tag=tag, name=name)
    finally:
        conn.close()


def _get_join_request(conn, clan_id, token_hash):
    return conn.execute(
        "SELECT * FROM clan_join_requests WHERE clan_id = ? AND token_hash = ?",
        (clan_id, token_hash),
    ).fetchone()


def _resolve_join_decision(conn, token_hash, from_username):
    """Gemeinsame Owner-Berechtigungsprüfung + Zielspieler-Auflösung für
    join_accept/join_decline. Gibt (clan, target, None) zurück, oder
    (None, None, (error_code, http_status))."""
    member = _get_member_row(conn, token_hash)
    if member is None:
        return None, None, ("not_in_clan", 403)
    clan = conn.execute("SELECT * FROM clans WHERE clan_id = ?", (member["clan_id"],)).fetchone()
    if clan["owner_token_hash"] != token_hash:
        return None, None, ("not_owner", 403)
    target = _get_player_by_username(conn, from_username)
    if target is None:
        return None, None, ("target_not_found", 404)
    return clan, target, None


@app.post("/api/clans/join_request")
def clan_join_request():
    """Ersetzt das frühere sofortige /api/clans/join - Beitritt braucht jetzt
    die Zustimmung des Owners (siehe join_accept/join_decline unten)."""
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    tag = (body.get("tag") or "").strip().upper()
    if not token or not tag:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404
        if _get_member_row(conn, token_hash) is not None:
            return jsonify(ok=False, error="already_in_clan"), 403
        clan = _get_clan_by_tag(conn, tag)
        if clan is None:
            return jsonify(ok=False, error="clan_not_found"), 404

        count = conn.execute(
            "SELECT COUNT(*) AS c FROM clan_members WHERE clan_id = ?", (clan["clan_id"],)
        ).fetchone()["c"]
        if count >= CLAN_MEMBER_CAP:
            return jsonify(ok=False, error="clan_full"), 403

        conn.execute("""
            INSERT INTO clan_join_requests (clan_id, token_hash, status, created_at)
            VALUES (?, ?, 'pending', datetime('now'))
            ON CONFLICT(clan_id, token_hash) DO UPDATE SET
                status = 'pending', created_at = excluded.created_at
        """, (clan["clan_id"], token_hash))
        conn.commit()
        return jsonify(ok=True, tag=clan["tag"], name=clan["name"])
    finally:
        conn.close()


@app.post("/api/clans/join_cancel")
def clan_join_cancel():
    """Zieht eine selbst gestellte, noch offene Beitrittsanfrage zurück."""
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    tag = (body.get("tag") or "").strip().upper()
    if not token or not tag:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404
        clan = _get_clan_by_tag(conn, tag)
        if clan is None:
            return jsonify(ok=True)
        conn.execute(
            "DELETE FROM clan_join_requests WHERE clan_id = ? AND token_hash = ? AND status = 'pending'",
            (clan["clan_id"], token_hash),
        )
        conn.commit()
        return jsonify(ok=True)
    finally:
        conn.close()


@app.post("/api/clans/join_requests_mine")
def clan_join_requests_mine():
    """Eigene noch offene Beitrittsanfragen - damit Clan-Suche/-Ansicht
    "Angefragt" statt "Beitreten" zeigen können."""
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    if not token:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404

        rows = conn.execute("""
            SELECT c.tag AS tag, c.name AS name, cjr.created_at AS created_at
            FROM clan_join_requests cjr JOIN clans c ON c.clan_id = cjr.clan_id
            WHERE cjr.token_hash = ? AND cjr.status = 'pending'
            ORDER BY cjr.created_at ASC
        """, (token_hash,)).fetchall()
        return jsonify(ok=True, requests=[
            {"tag": r["tag"], "name": r["name"], "created_at": r["created_at"]} for r in rows
        ])
    finally:
        conn.close()


@app.post("/api/clans/join_requests_incoming")
def clan_join_requests_incoming():
    """Owner-only: eingehende Beitrittsanfragen für den eigenen Clan."""
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    if not token:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404
        member = _get_member_row(conn, token_hash)
        if member is None:
            return jsonify(ok=False, error="not_in_clan"), 403
        clan = conn.execute("SELECT * FROM clans WHERE clan_id = ?", (member["clan_id"],)).fetchone()
        if clan["owner_token_hash"] != token_hash:
            return jsonify(ok=False, error="not_owner"), 403

        rows = conn.execute("""
            SELECT p.name AS name, p.username AS username, cjr.created_at AS created_at
            FROM clan_join_requests cjr JOIN players p ON p.token_hash = cjr.token_hash
            WHERE cjr.clan_id = ? AND cjr.status = 'pending'
            ORDER BY cjr.created_at ASC
        """, (clan["clan_id"],)).fetchall()
        return jsonify(ok=True, requests=[
            {"from_name": r["name"], "from_username": r["username"], "created_at": r["created_at"]}
            for r in rows
        ])
    finally:
        conn.close()


@app.post("/api/clans/join_accept")
def clan_join_accept():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    from_username = (body.get("from_username") or "").strip()
    if not token or not from_username:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404

        clan, target, error = _resolve_join_decision(conn, token_hash, from_username)
        if error:
            return jsonify(ok=False, error=error[0]), error[1]

        request_row = _get_join_request(conn, clan["clan_id"], target["token_hash"])
        if request_row is None or request_row["status"] != "pending":
            return jsonify(ok=False, error="no_pending_request"), 404

        # Zwei Races seit dem Stellen der Anfrage möglich: Zielspieler ist
        # inzwischen (anderswo) einem Clan beigetreten, oder dieser Clan ist
        # inzwischen voll - beides sauber melden statt auf die UNIQUE-
        # Constraint von clan_members.token_hash reinfallen zu lassen.
        if _get_member_row(conn, target["token_hash"]) is not None:
            conn.execute(
                "DELETE FROM clan_join_requests WHERE clan_id = ? AND token_hash = ?",
                (clan["clan_id"], target["token_hash"]),
            )
            conn.commit()
            return jsonify(ok=False, error="target_already_in_clan"), 403
        count = conn.execute(
            "SELECT COUNT(*) AS c FROM clan_members WHERE clan_id = ?", (clan["clan_id"],)
        ).fetchone()["c"]
        if count >= CLAN_MEMBER_CAP:
            return jsonify(ok=False, error="clan_full"), 403

        conn.execute(
            "INSERT INTO clan_members (clan_id, token_hash, joined_at) VALUES (?, ?, datetime('now'))",
            (clan["clan_id"], target["token_hash"]),
        )
        conn.execute(
            "DELETE FROM clan_join_requests WHERE clan_id = ? AND token_hash = ?",
            (clan["clan_id"], target["token_hash"]),
        )
        conn.commit()
        return jsonify(ok=True)
    finally:
        conn.close()


@app.post("/api/clans/join_decline")
def clan_join_decline():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    from_username = (body.get("from_username") or "").strip()
    if not token or not from_username:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404

        clan, target, error = _resolve_join_decision(conn, token_hash, from_username)
        if error:
            return jsonify(ok=False, error=error[0]), error[1]

        conn.execute(
            "DELETE FROM clan_join_requests WHERE clan_id = ? AND token_hash = ?",
            (clan["clan_id"], target["token_hash"]),
        )
        conn.commit()
        return jsonify(ok=True)
    finally:
        conn.close()


@app.post("/api/clans/leave")
def clan_leave():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    if not token:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404
        if _get_member_row(conn, token_hash) is None:
            return jsonify(ok=False, error="not_in_clan"), 403

        disbanded = _leave_clan_if_member(conn, token_hash)
        conn.commit()
        return jsonify(ok=True, disbanded=disbanded)
    finally:
        conn.close()


@app.post("/api/clans/kick")
def clan_kick():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    target_username = (body.get("target_username") or "").strip()
    if not token or not target_username:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404
        member = _get_member_row(conn, token_hash)
        if member is None:
            return jsonify(ok=False, error="not_in_clan"), 403
        clan = conn.execute("SELECT * FROM clans WHERE clan_id = ?", (member["clan_id"],)).fetchone()
        if clan["owner_token_hash"] != token_hash:
            return jsonify(ok=False, error="not_owner"), 403

        target = _get_player_by_username(conn, target_username)
        if target is None:
            return jsonify(ok=False, error="target_not_found"), 404
        if target["token_hash"] == token_hash:
            return jsonify(ok=False, error="bad_request"), 400
        target_member = conn.execute(
            "SELECT 1 FROM clan_members WHERE clan_id = ? AND token_hash = ?",
            (clan["clan_id"], target["token_hash"]),
        ).fetchone()
        if target_member is None:
            return jsonify(ok=False, error="target_not_member"), 404

        conn.execute(
            "DELETE FROM clan_members WHERE clan_id = ? AND token_hash = ?",
            (clan["clan_id"], target["token_hash"]),
        )
        conn.commit()
        return jsonify(ok=True)
    finally:
        conn.close()


@app.get("/api/clans/search")
def clan_search():
    q = request.args.get("q", "").strip()
    if not q:
        return jsonify(ok=True, results=[])
    conn = db.get_connection()
    try:
        rows = conn.execute("""
            SELECT c.tag AS tag, c.name AS name, COUNT(cm.token_hash) AS member_count
            FROM clans c LEFT JOIN clan_members cm ON cm.clan_id = c.clan_id
            WHERE c.tag LIKE ? OR c.name LIKE ?
            GROUP BY c.clan_id ORDER BY c.name LIMIT 25
        """, (f"%{q}%", f"%{q}%")).fetchall()
    finally:
        conn.close()
    return jsonify(ok=True, results=[
        {"tag": r["tag"], "name": r["name"], "member_count": r["member_count"]} for r in rows
    ])


@app.post("/api/clans/mine")
def clan_mine():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    if not token:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404
        member = _get_member_row(conn, token_hash)
        if member is None:
            return jsonify(ok=True, in_clan=False)

        clan = conn.execute("SELECT * FROM clans WHERE clan_id = ?", (member["clan_id"],)).fetchone()
        members = conn.execute("""
            SELECT p.name AS name, p.username AS username, p.token_hash AS token_hash, cm.joined_at AS joined_at
            FROM clan_members cm JOIN players p ON p.token_hash = cm.token_hash
            WHERE cm.clan_id = ? ORDER BY cm.joined_at ASC
        """, (clan["clan_id"],)).fetchall()
        owner_row = next((m for m in members if m["token_hash"] == clan["owner_token_hash"]), None)
        return jsonify(ok=True, in_clan=True, tag=clan["tag"], name=clan["name"],
                        is_owner=(clan["owner_token_hash"] == token_hash),
                        owner_name=owner_row["name"] if owner_row else None,
                        owner_username=owner_row["username"] if owner_row else None,
                        members=[{"name": m["name"], "username": m["username"], "joined_at": m["joined_at"]}
                                 for m in members])
    finally:
        conn.close()


@app.get("/api/clans/<tag>")
def clan_view(tag):
    conn = db.get_connection()
    try:
        clan = _get_clan_by_tag(conn, tag.strip().upper())
        if clan is None:
            return jsonify(ok=False, error="not_found"), 404

        owner = conn.execute(
            "SELECT name, username FROM players WHERE token_hash = ?", (clan["owner_token_hash"],)
        ).fetchone()
        members = conn.execute("""
            SELECT p.name AS name, p.username AS username, cm.joined_at AS joined_at
            FROM clan_members cm JOIN players p ON p.token_hash = cm.token_hash
            WHERE cm.clan_id = ? ORDER BY cm.joined_at ASC
        """, (clan["clan_id"],)).fetchall()
        return jsonify(ok=True, clan={
            "tag": clan["tag"],
            "name": clan["name"],
            "owner_name": owner["name"] if owner else None,
            "owner_username": owner["username"] if owner else None,
            "member_count": len(members),
            "members": [{"name": m["name"], "username": m["username"], "joined_at": m["joined_at"]}
                        for m in members],
        })
    finally:
        conn.close()


# =============================
# Chat (Freunde-DMs + Clan-Kanal)
# =============================

MESSAGE_MAX_LENGTH = 300
CHAT_RATE_LIMIT_SECONDS = 2


def _resolve_conversation(conn, token_hash, conversation_type, target):
    """Prüft Berechtigung und berechnet die conversation_id serverseitig -
    niemals vom Client übernommen, damit niemand Nachrichten in eine fremde
    Unterhaltung einschleusen kann. Gibt (conversation_id, error) zurück;
    error ist None bei Erfolg, sonst ein (error_code, http_status)-Tupel."""
    if conversation_type == "dm":
        other = _get_player_by_username(conn, target)
        if other is None:
            return None, ("target_not_found", 404)
        other_hash = other["token_hash"]
        friends = conn.execute(
            "SELECT 1 FROM friend_requests WHERE status = 'accepted' AND "
            "((from_token_hash = ? AND to_token_hash = ?) OR (from_token_hash = ? AND to_token_hash = ?))",
            (token_hash, other_hash, other_hash, token_hash),
        ).fetchone()
        if friends is None:
            return None, ("not_friends", 403)
        pair = sorted([token_hash, other_hash])
        return f"{pair[0]}:{pair[1]}", None
    elif conversation_type == "clan":
        clan = _get_clan_by_tag(conn, target.strip().upper())
        if clan is None:
            return None, ("target_not_found", 404)
        member = conn.execute(
            "SELECT 1 FROM clan_members WHERE clan_id = ? AND token_hash = ?",
            (clan["clan_id"], token_hash),
        ).fetchone()
        if member is None:
            return None, ("not_clan_member", 403)
        return str(clan["clan_id"]), None
    return None, ("bad_request", 400)


@app.post("/api/messages/send")
def messages_send():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    conversation_type = (body.get("conversation_type") or "").strip()
    target = (body.get("target") or "").strip()
    text = body.get("body") or ""
    if not token or conversation_type not in ("dm", "clan") or not target or not text.strip():
        return jsonify(ok=False, error="bad_request"), 400
    if len(text) > MESSAGE_MAX_LENGTH:
        return jsonify(ok=False, error="message_too_long"), 400
    if not is_text_allowed(text):
        return jsonify(ok=False, error="message_filtered"), 400

    token_hash = _hash_secret(token)
    if _rate_limited("message", token_hash, CHAT_RATE_LIMIT_SECONDS):
        return jsonify(ok=False, error="rate_limited"), 429

    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404

        conversation_id, error = _resolve_conversation(conn, token_hash, conversation_type, target)
        if error:
            return jsonify(ok=False, error=error[0]), error[1]

        cur = conn.execute("""
            INSERT INTO messages (conversation_type, conversation_id, sender_token_hash, body, created_at)
            VALUES (?, ?, ?, ?, datetime('now'))
        """, (conversation_type, conversation_id, token_hash, text))
        conn.commit()
        row = conn.execute("SELECT created_at FROM messages WHERE id = ?", (cur.lastrowid,)).fetchone()
        return jsonify(ok=True, id=cur.lastrowid, created_at=row["created_at"])
    finally:
        conn.close()


@app.post("/api/messages/list")
def messages_list():
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    conversation_type = (body.get("conversation_type") or "").strip()
    target = (body.get("target") or "").strip()
    since_id = body.get("since_id") or 0
    if not token or conversation_type not in ("dm", "clan") or not target:
        return jsonify(ok=False, error="bad_request"), 400
    if not isinstance(since_id, int) or isinstance(since_id, bool):
        since_id = 0

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404

        conversation_id, error = _resolve_conversation(conn, token_hash, conversation_type, target)
        if error:
            return jsonify(ok=False, error=error[0]), error[1]

        if since_id > 0:
            # Inkrementelles Polling - alles Neue seit dem letzten gesehenen id,
            # aufsteigend sortiert (natürliche Anhänge-Reihenfolge fürs Chat-Fenster).
            rows = conn.execute("""
                SELECT m.id AS id, p.name AS sender_name, m.body AS body, m.created_at AS created_at
                FROM messages m JOIN players p ON p.token_hash = m.sender_token_hash
                WHERE m.conversation_type = ? AND m.conversation_id = ? AND m.id > ?
                ORDER BY m.id ASC LIMIT 200
            """, (conversation_type, conversation_id, since_id)).fetchall()
        else:
            # Erstes Laden eines Chats: die letzten 50 Nachrichten, nicht die
            # ältesten - sonst würde ein Chat mit mehr als 200 Nachrichten
            # Gesamtverlauf beim Öffnen die uralte Historie zeigen statt der
            # aktuellen (ASC+LIMIT ohne since_id würde immer bei id 1 anfangen).
            # Danach wieder aufsteigend sortiert, damit die Anzeige von oben
            # nach unten chronologisch bleibt.
            rows = conn.execute("""
                SELECT id, sender_name, body, created_at FROM (
                    SELECT m.id AS id, p.name AS sender_name, m.body AS body, m.created_at AS created_at
                    FROM messages m JOIN players p ON p.token_hash = m.sender_token_hash
                    WHERE m.conversation_type = ? AND m.conversation_id = ?
                    ORDER BY m.id DESC LIMIT 50
                ) ORDER BY id ASC
            """, (conversation_type, conversation_id)).fetchall()
        return jsonify(ok=True, messages=[
            {"id": r["id"], "sender_name": r["sender_name"], "body": r["body"], "created_at": r["created_at"]}
            for r in rows
        ])
    finally:
        conn.close()


@app.post("/api/messages/mark_read")
def messages_mark_read():
    """Merkt sich die höchste gesehene Nachrichten-id einer Unterhaltung für
    diesen Spieler - Grundlage für /api/notifications/summary weiter unten.
    Nutzt _resolve_conversation() (nicht dupliziert) fürs selbe Berechtigungs-
    /ID-Modell wie send/list; ein Client-seitig mitgeschicktes last_read_id
    würde hier nie vertraut, es wird immer serverseitig aus messages neu
    berechnet."""
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    conversation_type = (body.get("conversation_type") or "").strip()
    target = (body.get("target") or "").strip()
    if not token or conversation_type not in ("dm", "clan") or not target:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404

        conversation_id, error = _resolve_conversation(conn, token_hash, conversation_type, target)
        if error:
            return jsonify(ok=False, error=error[0]), error[1]

        latest = conn.execute(
            "SELECT MAX(id) AS max_id FROM messages WHERE conversation_type = ? AND conversation_id = ?",
            (conversation_type, conversation_id),
        ).fetchone()
        max_id = latest["max_id"] or 0
        conn.execute("""
            INSERT INTO read_markers (token_hash, conversation_type, conversation_id, last_read_id)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(token_hash, conversation_type, conversation_id) DO UPDATE SET
                last_read_id = MAX(last_read_id, excluded.last_read_id)
        """, (token_hash, conversation_type, conversation_id, max_id))
        conn.commit()
        return jsonify(ok=True, last_read_id=max_id)
    finally:
        conn.close()


@app.post("/api/notifications/summary")
def notifications_summary():
    """Ein gebündelter Endpunkt statt N Einzelabfragen vom Client (ein Request
    pro Freund waere bei einem selbstgehosteten Heimserver unnötig teuer) -
    serverseitig sind das mehrere kleine, günstige lokale Queries innerhalb
    derselben Connection, das ist etwas ganz anderes als N Netzwerk-
    Roundtrips und muss nicht zu einer einzigen Mega-Query verbogen werden."""
    body = request.get_json(silent=True) or {}
    token = body.get("token") or ""
    if not token:
        return jsonify(ok=False, error="bad_request"), 400

    token_hash = _hash_secret(token)
    conn = db.get_connection()
    try:
        me = _get_player_by_token(conn, token_hash)
        if me is None:
            return jsonify(ok=False, error="unknown_player"), 404

        pending_friend_requests = conn.execute(
            "SELECT COUNT(*) AS c FROM friend_requests WHERE to_token_hash = ? AND status = 'pending'",
            (token_hash,),
        ).fetchone()["c"]

        conversation_ids = []
        friend_hashes = conn.execute("""
            SELECT CASE WHEN from_token_hash = ? THEN to_token_hash ELSE from_token_hash END AS other_hash
            FROM friend_requests
            WHERE status = 'accepted' AND (from_token_hash = ? OR to_token_hash = ?)
        """, (token_hash, token_hash, token_hash)).fetchall()
        for row in friend_hashes:
            pair = sorted([token_hash, row["other_hash"]])
            conversation_ids.append(("dm", f"{pair[0]}:{pair[1]}"))

        member = _get_member_row(conn, token_hash)
        if member is not None:
            conversation_ids.append(("clan", str(member["clan_id"])))

        unread_conversations = 0
        for conversation_type, conversation_id in conversation_ids:
            latest = conn.execute(
                "SELECT MAX(id) AS max_id FROM messages WHERE conversation_type = ? AND conversation_id = ?",
                (conversation_type, conversation_id),
            ).fetchone()
            max_id = latest["max_id"]
            if max_id is None:
                continue
            marker = conn.execute(
                "SELECT last_read_id FROM read_markers WHERE token_hash = ? AND conversation_type = ? "
                "AND conversation_id = ?",
                (token_hash, conversation_type, conversation_id),
            ).fetchone()
            last_read_id = marker["last_read_id"] if marker else 0
            if max_id > last_read_id:
                unread_conversations += 1

        return jsonify(ok=True, pending_friend_requests=pending_friend_requests,
                        unread_conversations=unread_conversations)
    finally:
        conn.close()


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000, threaded=True)
