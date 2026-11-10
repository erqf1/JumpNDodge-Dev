//! Online-Leaderboard-Client (Python: Abschnitt "Online-Leaderboard (Client)").
//! Alles best-effort: Fehler liefern None, Anfragen laufen in eigenen Threads,
//! die Antworten werden im Hauptthread (poll_online) an die Callbacks gegeben.

use super::state::{OnDone, G};
use crate::util::*;
use serde_json::{json, Value};
use std::sync::mpsc;
use std::time::Duration;
use unicode_normalization::UnicodeNormalization;

pub const ONLINE_ENABLED: bool = true;
pub const ONLINE_SERVER_URL_DEFAULT: &str = "https://leaderboard.jumpndodge.download";

/// Ergebnis eines Hintergrund-Requests: (id, Antwort, Server erreichbar?)
pub type OnlineMsg = (u64, Option<Value>, Option<bool>);

pub fn online_server_url() -> String {
    std::env::var("JND_SERVER_URL").unwrap_or_else(|_| ONLINE_SERVER_URL_DEFAULT.to_string())
}

/// _online_request() - blockierend, nur im Hintergrund-Thread aufrufen.
pub fn online_request_blocking(method: &str, path: &str, payload: Option<&Value>, timeout: u64) -> (Option<Value>, Option<bool>) {
    if !ONLINE_ENABLED { return (None, None); }
    let url = format!("{}{}", online_server_url().trim_end_matches('/'), path);
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(timeout)).build();
    let req = agent.request(method, &url)
        .set("Accept", "application/json")
        .set("User-Agent", "JumpNDodge-Game-Client/1.0");
    let res = match payload {
        Some(p) => req.set("Content-Type", "application/json").send_string(&crate::crypto::py_json_dumps(p)),
        None => req.call(),
    };
    match res {
        Ok(r) => match r.into_string().ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) {
            Some(v) => (Some(v), Some(true)),
            None => (None, Some(false)),
        },
        Err(ureq::Error::Status(_code, r)) => {
            // Server hat geantwortet (Fehlerstatus) - erreichbar
            let v = r.into_string().ok().and_then(|s| serde_json::from_str::<Value>(&s).ok());
            (v, Some(true))
        }
        Err(_) => (None, Some(false)),
    }
}

const DEFAULT_BANNED_WORDS: [&str; 8] = ["fuck", "shit", "bitch", "nigger", "faggot", "hurensohn", "wichser", "arschloch"];

pub fn online_normalize_name(text: &str) -> String {
    let t: String = text.to_lowercase();
    let t: String = t.nfkd().filter(|c| !unicode_normalization::char::is_combining_mark(*c)).collect();
    let t: String = t.chars().map(|c| match c { '0' => 'o', '1' => 'i', '3' => 'e', '4' => 'a', '5' => 's', '7' => 't', '@' => 'a', '$' => 's', _ => c }).collect();
    t.chars().filter(|c| c.is_alphanumeric()).collect()
}

impl G {
    pub fn _mark_server_reachable(&mut self, ok: bool) { self._server_reachable = Some(ok); }

    /// _online_request_async(method, path, payload, on_done)
    pub fn online_request_async(&mut self, method: &str, path: &str, payload: Option<Value>, on_done: Option<OnDone>) {
        self._online_next_id += 1;
        let id = self._online_next_id;
        if let Some(cb) = on_done { self._online_callbacks.insert(id, cb); }
        let tx: mpsc::Sender<OnlineMsg> = self._online_ch.tx.clone();
        let (m, p) = (method.to_string(), path.to_string());
        self.debug_log(&format!("-> {} {}", method, path));
        std::thread::spawn(move || {
            let (res, reach) = online_request_blocking(&m, &p, payload.as_ref(), 5);
            let _ = tx.send((id, res, reach));
        });
    }

    /// Antworten der Hintergrund-Requests abholen und Callbacks im Hauptthread ausführen
    pub fn poll_online(&mut self) {
        loop {
            let msg = match self._online_ch.rx.try_recv() { Ok(m) => m, Err(_) => break };
            let (id, res, reach) = msg;
            if let Some(r) = reach { self._mark_server_reachable(r); }
            if let Some(cb) = self._online_callbacks.remove(&id) {
                cb(self, res);
            }
        }
    }

    pub fn ensure_online_token(&mut self) -> String {
        if self.online_token.is_none() {
            use base64::Engine;
            let bytes: [u8; 24] = rand::random();
            let tok = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
            self.online_token = Some(tok);
            self.save_current_profile();
        }
        self.online_token.clone().unwrap()
    }

    pub fn get_cached_banned_words(&mut self) -> Vec<String> {
        if self._banned_words_cache.is_none() {
            self._banned_words_cache = Some(DEFAULT_BANNED_WORDS.iter().map(|s| s.to_string()).collect());
            self.online_request_async("GET", "/api/banned_words", None, Some(Box::new(|g, result| {
                if let Some(r) = result {
                    if r.truthy("ok") {
                        if let Some(Value::Array(a)) = r.gv("words") {
                            g._banned_words_cache = Some(a.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                        }
                    }
                }
            })));
        }
        self._banned_words_cache.clone().unwrap()
    }

    pub fn is_name_allowed(&mut self, name: &str) -> bool {
        if name.trim().is_empty() { return false; }
        let normalized = online_normalize_name(name);
        if normalized.is_empty() { return false; }
        for word in self.get_cached_banned_words() {
            let needle = online_normalize_name(&word);
            if !needle.is_empty() && normalized.contains(&needle) { return false; }
        }
        true
    }

    pub fn submit_score_online(&mut self, mode: &str, difficulty: &str, score: i64) {
        if !ONLINE_ENABLED || self.current_profile_name.is_none() { return; }
        let token = self.ensure_online_token();
        let sent_name = self.current_profile_name.clone().unwrap();
        let payload = json!({
            "name": sent_name,
            "token": token,
            "mode": mode,
            "difficulty": difficulty,
            "score": score,
            "powers_used": self.powers_state.garr("equipped"),
            "profile_snapshot": {
                "stats": Value::Object(self.stats.clone()),
                "owned_powers": self.powers_state.garr("owned"),
                "equipped_powers": self.powers_state.garr("equipped"),
                "cosmetics": Value::Object(self.cosmetics_state.clone()),
                "player_color": self.settings.gs("player_color", "blue"),
            },
        });
        self.online_request_async("POST", "/api/submit", Some(payload), Some(Box::new(move |g, result| {
            if let Some(r) = result {
                if r.truthy("ok") {
                    if let Some(confirmed) = r.gos("name") {
                        if !confirmed.is_empty() && confirmed != sent_name {
                            g._submit_pending_local_name = Some(confirmed);
                        }
                    }
                }
            }
        })));
    }

    pub fn fetch_leaderboard(&mut self, mode: &str, difficulty: &str, limit: i64, usernames: Option<Vec<String>>, on_done: Option<OnDone>) {
        let mut path = format!("/api/leaderboard?mode={}&difficulty={}&limit={}", quote(mode), quote(difficulty), limit);
        if let Some(u) = usernames {
            if !u.is_empty() { path += &format!("&usernames={}", quote(&u.join(","))); }
        }
        self.online_request_async("GET", &path, None, on_done);
    }

    pub fn fetch_my_rank(&mut self, mode: &str, difficulty: &str, on_done: Option<OnDone>) {
        if self.current_profile_name.is_none() { return; }
        let token = self.ensure_online_token();
        self.online_request_async("POST", "/api/my_rank", Some(json!({"token": token, "mode": mode, "difficulty": difficulty})), on_done);
    }

    pub fn search_players_online(&mut self, query: &str, on_done: Option<OnDone>) {
        let path = format!("/api/players/search?q={}", quote(query));
        self.online_request_async("GET", &path, None, on_done);
    }

    pub fn fetch_public_profile(&mut self, username: &str, on_done: Option<OnDone>) {
        let path = format!("/api/players/{}", quote(username));
        self.online_request_async("GET", &path, None, on_done);
    }

    fn token_post(&mut self, path: &str, mut extra: serde_json::Map<String, Value>, on_done: Option<OnDone>) {
        if self.current_profile_name.is_none() { return; }
        let token = self.ensure_online_token();
        let mut p = serde_json::Map::new();
        p.insert("token".into(), Value::String(token));
        p.append(&mut extra);
        self.online_request_async("POST", path, Some(Value::Object(p)), on_done);
    }

    pub fn redeem_verified_key_online(&mut self, key: &str, on_done: Option<OnDone>) {
        self.token_post("/api/redeem_key", obj_of(&json!({"key": key.trim()})), on_done);
    }
    pub fn redeem_code_online(&mut self, code: &str, on_done: Option<OnDone>) {
        self.token_post("/api/redeem_code", obj_of(&json!({"code": code.trim()})), on_done);
    }
    pub fn rename_profile_online(&mut self, new_name: &str, on_done: Option<OnDone>) {
        self.token_post("/api/rename", obj_of(&json!({"new_name": new_name.trim()})), on_done);
    }
    pub fn set_username_online(&mut self, username: &str, on_done: Option<OnDone>) {
        let name = self.current_profile_name.clone().unwrap_or_default();
        self.token_post("/api/set_username", obj_of(&json!({"name": name, "username": username.trim()})), on_done);
    }
    pub fn ping_server_online(&mut self, on_done: Option<OnDone>) {
        self.online_request_async("GET", "/api/ping", None, on_done);
    }
    pub fn send_friend_request_online(&mut self, to_username: &str, on_done: Option<OnDone>) {
        self.token_post("/api/friends/request", obj_of(&json!({"to_username": to_username})), on_done);
    }
    pub fn respond_friend_request_online(&mut self, from_username: &str, accept: bool, on_done: Option<OnDone>) {
        if accept {
            self.token_post("/api/friends/accept", obj_of(&json!({"from_username": from_username})), on_done);
        } else {
            self.token_post("/api/friends/decline", obj_of(&json!({"other_username": from_username})), on_done);
        }
    }
    pub fn remove_friend_online(&mut self, username: &str, on_done: Option<OnDone>) {
        self.token_post("/api/friends/remove", obj_of(&json!({"other_username": username})), on_done);
    }
    pub fn fetch_friends_list(&mut self, username: &str, on_done: Option<OnDone>) {
        let path = format!("/api/friends/list?username={}", quote(username));
        self.online_request_async("GET", &path, None, on_done);
    }
    pub fn fetch_incoming_friend_requests(&mut self, on_done: Option<OnDone>) {
        self.token_post("/api/friends/incoming", Default::default(), on_done);
    }
    pub fn fetch_conversations_list(&mut self, on_done: Option<OnDone>) {
        self.token_post("/api/messages/conversations", Default::default(), on_done);
    }
    pub fn create_clan_online(&mut self, tag: &str, name: &str, on_done: Option<OnDone>) {
        self.token_post("/api/clans/create", obj_of(&json!({"tag": tag.trim(), "name": name.trim()})), on_done);
    }
    pub fn request_join_clan_online(&mut self, tag: &str, on_done: Option<OnDone>) {
        self.token_post("/api/clans/join_request", obj_of(&json!({"tag": tag.trim()})), on_done);
    }
    pub fn cancel_join_clan_online(&mut self, tag: &str, on_done: Option<OnDone>) {
        self.token_post("/api/clans/join_cancel", obj_of(&json!({"tag": tag.trim()})), on_done);
    }
    pub fn fetch_my_clan_join_requests(&mut self, on_done: Option<OnDone>) {
        self.token_post("/api/clans/join_requests_mine", Default::default(), on_done);
    }
    pub fn fetch_clan_incoming_join_requests(&mut self, on_done: Option<OnDone>) {
        self.token_post("/api/clans/join_requests_incoming", Default::default(), on_done);
    }
    pub fn respond_clan_join_request_online(&mut self, from_username: &str, accept: bool, on_done: Option<OnDone>) {
        let path = if accept { "/api/clans/join_accept" } else { "/api/clans/join_decline" };
        self.token_post(path, obj_of(&json!({"from_username": from_username})), on_done);
    }
    pub fn leave_clan_online(&mut self, on_done: Option<OnDone>) {
        self.token_post("/api/clans/leave", Default::default(), on_done);
    }
    pub fn kick_clan_member_online(&mut self, target_username: &str, on_done: Option<OnDone>) {
        self.token_post("/api/clans/kick", obj_of(&json!({"target_username": target_username})), on_done);
    }
    pub fn fetch_clan_info(&mut self, tag: &str, on_done: Option<OnDone>) {
        let path = format!("/api/clans/{}", quote(tag));
        self.online_request_async("GET", &path, None, on_done);
    }
    pub fn fetch_my_clan(&mut self, on_done: Option<OnDone>) {
        self.token_post("/api/clans/mine", Default::default(), on_done);
    }
    pub fn search_clans_online(&mut self, query: &str, on_done: Option<OnDone>) {
        let path = format!("/api/clans/search?q={}", quote(query));
        self.online_request_async("GET", &path, None, on_done);
    }
    pub fn send_chat_message_online(&mut self, conversation_type: &str, target: &str, body: &str, on_done: Option<OnDone>) {
        self.token_post("/api/messages/send", obj_of(&json!({"conversation_type": conversation_type, "target": target, "body": body})), on_done);
    }
    pub fn fetch_chat_messages(&mut self, conversation_type: &str, target: &str, since_id: i64, on_done: Option<OnDone>) {
        self.token_post("/api/messages/list", obj_of(&json!({"conversation_type": conversation_type, "target": target, "since_id": since_id})), on_done);
    }
    pub fn mark_chat_read_online(&mut self, conversation_type: &str, target: &str, on_done: Option<OnDone>) {
        self.token_post("/api/messages/mark_read", obj_of(&json!({"conversation_type": conversation_type, "target": target})), on_done);
    }
    pub fn fetch_notifications_summary(&mut self, on_done: Option<OnDone>) {
        self.token_post("/api/notifications/summary", Default::default(), on_done);
    }

    pub fn _refresh_notifications_if_due(&mut self) {
        if self.current_profile_username.is_none() { return; }
        let now = self.get_ticks();
        if now - self._notif_last_checked_ms < 15000 { return; }
        self._notif_last_checked_ms = now;
        self.fetch_notifications_summary(Some(Box::new(|g, result| {
            if let Some(r) = result {
                if r.truthy("ok") {
                    g._notif_pending_friend_requests = r.gi("pending_friend_requests", 0);
                    g._notif_pending_clan_requests = r.gi("pending_clan_join_requests", 0);
                    g._notif_unread_conversations = r.gi("unread_conversations", 0);
                }
            }
        })));
    }

    pub fn _draw_notification_badge(&mut self, surface: &crate::pg::Surface, cx: i32, cy: i32, count: i64) {
        use crate::pg::draw;
        let radius = 11;
        draw::circle(surface, (220, 60, 60), (cx, cy), radius, 0);
        draw::circle(surface, (255, 255, 255), (cx, cy), radius, 2);
        let txt = if count < 10 { count.to_string() } else { "9+".to_string() };
        let txt_s = self.badge_font.render(&txt, true, (255, 255, 255));
        surface.blit(&txt_s, (cx - txt_s.get_width() / 2, cy - txt_s.get_height() / 2));
    }

    pub fn _flush_pending_online_deletions(&mut self) {
        let pending = self._load_profiles_file().gstrs("_pending_online_deletions");
        for token in pending {
            let t2 = token.clone();
            self.online_request_async("POST", "/api/delete_account", Some(json!({"token": token})), Some(Box::new(move |g, result| {
                if let Some(r) = result {
                    if r.truthy("ok") { g._confirmed_online_deletions.push(t2); }
                }
            })));
        }
    }

    pub fn _apply_confirmed_online_deletions(&mut self) {
        if self._confirmed_online_deletions.is_empty() { return; }
        let mut pdata = self._load_profiles_file();
        let mut pending = pdata.gstrs("_pending_online_deletions");
        for token in &self._confirmed_online_deletions {
            pending.retain(|t| t != token);
        }
        pdata.insert("_pending_online_deletions".into(), json!(pending));
        self._save_profiles_file(pdata);
        self._confirmed_online_deletions.clear();
    }

    pub fn rename_current_profile_local(&mut self, new_name: &str) {
        let Some(old_name) = self.current_profile_name.clone() else { return; };
        if old_name == new_name { return; }
        let mut pdata = self._load_profiles_file();
        let mut profiles = pdata.gobj("profiles");
        let Some(entry) = profiles.shift_remove(&old_name) else { return; };
        let mut entry = obj_of(&entry);
        entry.insert("name".into(), json!(new_name));
        profiles.insert(new_name.to_string(), Value::Object(entry));
        pdata.insert("profiles".into(), Value::Object(profiles));
        if pdata.gos("active").as_deref() == Some(old_name.as_str()) { pdata.insert("active".into(), json!(new_name)); }
        if pdata.gos("auto_start").as_deref() == Some(old_name.as_str()) { pdata.insert("auto_start".into(), json!(new_name)); }
        self._save_profiles_file(pdata);
        self.current_profile_name = Some(new_name.to_string());
    }

    pub fn check_local_verified_key_file(&mut self) {
        let key_path = self._DATA_DIR.join("verified_key.txt");
        if !key_path.is_file() { return; }
        let Ok(key) = std::fs::read_to_string(&key_path) else { return; };
        let key = key.trim().to_string();
        if !key.is_empty() { self.redeem_verified_key_online(&key, None); }
    }
}
