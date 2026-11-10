//! Der globale Spielzustand: jede globale Variable aus JumpNDodge.py ist hier ein Feld
//! mit demselben Namen.
#![allow(non_snake_case)]

use crate::pg::*;
use crate::util::Obj;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;

pub type OnDone = Box<dyn FnOnce(&mut G, Option<Value>)>;
pub type Cb = Box<dyn FnOnce(&mut G)>;

/// Sound-Handles (Python übergibt die Sound-Objekte direkt)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Snd { MenuClick, Click, ClickBack, Hit, GameOver, Heartbeat, OneUp, None }

pub struct Chan<T> { pub tx: mpsc::Sender<T>, pub rx: mpsc::Receiver<T> }
impl<T> Default for Chan<T> { fn default() -> Self { let (tx, rx) = mpsc::channel(); Chan { tx, rx } } }

#[derive(Default)]
pub struct TimeCache {
    pub checked_at: Option<f64>,
    pub server_datetime: Option<chrono::NaiveDateTime>,
    pub has_internet: bool,
}

#[derive(Clone, Debug)]
pub struct Overlay {
    pub text: String,
    pub color: (i32, i32, i32),
    pub until: f64,
    pub start: f64,
    pub pos: Option<(i32, i32)>,
}

#[derive(Default)]
pub struct G {
    // ---- pygame / Anzeige ----
    pub display: Display,
    pub mixer: Mixer,
    pub base_surface: Surface,
    pub WIDTH: i32,
    pub HEIGHT: i32,
    pub scale_x: f64,
    pub scale_y: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub _last_mouse_phys_pos: Option<(i32, i32)>,
    pub _kb_nav_active: bool,
    pub _bg_stars: Vec<[f64; 7]>,
    pub _window_locked_for_gameplay: bool,
    pub _fade_alpha: i32,
    pub _fade_dir: i32,
    pub _fade_callback: Option<Cb>,
    pub _frame_counter: u64,
    pub _gui_scale_dragging: bool,
    pub _custom_speed_dragging: bool,
    pub _custom_min_delay_dragging: bool,
    pub _custom_max_delay_dragging: bool,
    pub _ds_slider_edit: Option<i32>,
    pub admin_power_rects: Vec<(Rect, String)>,

    // ---- Schriften ----
    pub font: Font,
    pub menu_font: Font,
    pub small_font: Font,
    pub mono_font: Font,
    pub badge_font: Font,
    pub tooltip_font: Font,
    pub tab_font: Font,

    // ---- Pfade / Spielstand ----
    pub BASE_DIR: PathBuf,
    pub _DATA_DIR: PathBuf,
    pub _GAMEDATA_FILE: PathBuf,
    pub _MODDED_DATA_FILE: PathBuf,
    pub _gamedata_cache: Obj,
    pub _modded_cache: Obj,
    pub _live_sections_ready: bool,
    pub stats: Obj,
    pub achievements: Obj,
    pub saved_scores: Obj,
    pub best_times: Obj,
    pub modded_scores: Obj,
    pub modded_best_times: Obj,
    pub modded_stats: Obj,
    pub settings: Obj,
    pub powers_state: Obj,
    pub cosmetics_state: Obj,
    pub sale_state: Obj,
    pub current_profile_name: Option<String>,
    pub _profile_save_lock: bool,
    pub online_token: Option<String>,
    pub current_profile_username: Option<String>,
    pub redeemed_codes_local: Vec<String>,
    pub friends_local: Vec<Value>,

    // ---- Sprache / Schwierigkeit / Modus ----
    pub languages: Vec<String>,
    pub texts: Obj,
    pub language_index: usize,
    pub difficulty_index: usize,
    pub current_game_mode: String,
    pub two_player_submode: String,
    pub klassik_mode: bool,
    pub last_export_message: String,

    // ---- Online ----
    pub _server_reachable: Option<bool>,
    pub _online_next_id: u64,
    pub _online_callbacks: HashMap<u64, OnDone>,
    pub _online_ch: Chan<crate::game::online::OnlineMsg>,
    pub _banned_words_cache: Option<Vec<String>>,
    pub _submit_pending_local_name: Option<String>,
    pub _confirmed_online_deletions: Vec<String>,
    pub _notif_pending_friend_requests: i64,
    pub _notif_pending_clan_requests: i64,
    pub _notif_unread_conversations: i64,
    pub _notif_last_checked_ms: i64,

    // ---- Zeit / Admin ----
    pub _time_cache: TimeCache,
    pub _time_ch: Chan<(bool, Option<chrono::NaiveDateTime>)>,
    pub _time_check_running: bool,
    pub _admin_keyfile_cache: (bool, f64),

    // ---- Sound ----
    pub sound_enabled: bool,
    pub menu_click_sound: Option<Sound>,
    pub click_sound: Option<Sound>,
    pub click_back_sound: Option<Sound>,
    pub hit_sound: Option<Sound>,
    pub gameover_sound: Option<Sound>,
    pub heartbeat_sound: Option<Sound>,
    pub oneup_sound: Option<Sound>,
    pub menu_music_path: Option<String>,
    pub ingame_music_path: Option<String>,
    pub INGAME_MUSIC_TRACKS: Vec<(String, String, String)>,
    pub ingame_music_paths: Vec<(String, Option<String>)>,
    pub _preview_channel: Option<Channel>,
    pub _preview_track_key: Option<String>,

    // ---- Menü-Navigation ----
    pub _nav_focus: HashMap<String, i64>,

    // ---- Overlays / Popups ----
    pub overlays: Vec<Overlay>,

    // ---- Bildschirm-Flags ----
    pub game_running: bool,
    pub in_menu: bool,

    pub in_difficulty_select: bool,
    pub showing_stats: bool,
    pub in_shop_menu: bool,
    pub in_loadout_menu: bool,
    pub in_profile_hub: bool,
    pub in_profile_select: bool,
    pub in_online_hub: bool,
    pub in_online_leaderboard: bool,
    pub in_friends_hub: bool,
    pub in_player_search: bool,
    pub _search_query: String,
    pub _search_results: Option<Vec<Value>>,
    pub _search_loading: bool,
    pub _player_search_return_screen: String,
    pub in_clan_hub: bool,
    pub in_public_profile: bool,
    pub _public_profile_username: Option<String>,
    pub _public_profile_data: Option<Value>,
    pub _public_profile_loading: bool,
    pub _public_profile_return_screen: String,
    pub _friend_action_pending: Option<String>,
    pub _public_profile_friend_msg: String,
    pub in_more_menu: bool,
    pub in_redeem_code: bool,
    pub _redeem_code_text: String,
    pub _redeem_code_msg: String,
    pub _redeem_code_busy: bool,
    pub in_rename_profile: bool,
    pub _rename_text: String,
    pub _rename_msg: String,
    pub _rename_busy: bool,
    pub in_loading: bool,
    pub loading_progress: f64,
    pub loading_start_time: i64,
    pub loading_to_daily_rewards: bool,
    pub loading_duration: i64,
    pub in_admin_login: bool,
    pub admin_pwd_input: String,
    pub admin_msg: String,
    pub in_settings_menu: bool,
    pub buttons: Vec<crate::game::menus::Button>,
    pub in_classic_game: bool,
    pub in_controls_menu: bool,
    pub _ctrl_listening: Option<String>,
    pub _ctrl_scroll: i32,
    pub in_settings_more_menu: bool,
    pub in_experimental_menu: bool,
    pub in_settings_sound_menu: bool,
    pub in_help: bool,
    pub in_daily_rewards: bool,
    pub in_cosmetic_shop: bool,
    pub in_cosmetic_inventory: bool,
    pub help_scroll: i32,
    pub help_lines: Vec<String>,
    pub help_lang: Option<String>,
    pub tutorial_step: usize,
    pub in_tutorial: bool,
    pub in_language_select: bool,
    pub _lang_from_settings: bool,
    pub in_shop: bool,
    pub in_loadout: bool,
    pub in_admin_panel: bool,
    pub in_achievements_screen: bool,
    pub in_profile_name_input: bool,
    pub _profile_name_text: String,
    pub _profile_name_msg: String,
    pub _profile_from_settings: bool,
    pub _lb_request_id: u64,
    pub _lb_my_rank: Option<(Option<i64>, Option<i64>)>,
    pub _lb_difficulty_idx: usize,
    pub _lb_friends_only: bool,
    pub _lb_loading: bool,
    pub _lb_entries: Option<Vec<Value>>,
    pub _lb_row_rects: Vec<(Rect, Option<String>)>,
    pub _friends_local_pending: Option<Vec<Value>>,
    pub _search_request_id: u64,
    pub _search_row_rects: Vec<(Rect, Option<String>)>,
    pub in_username_gate: bool,
    pub _username_gate_checking: bool,
    pub _username_gate_reachable: Option<bool>,
    pub _username_gate_text: String,
    pub _username_gate_msg: String,
    pub _username_gate_busy: bool,
    pub _username_gate_pending_success: Option<String>,
    // @@FELDER@@ (neue Felder werden hier eingefügt)
}
