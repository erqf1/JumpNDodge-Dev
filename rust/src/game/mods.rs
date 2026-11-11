//! Lua-Mods (Rust Edition): sicher in einer Sandbox.
//!
//! Mods liegen als `*.lua` im Ordner `mods/` neben dem Spielstand. Jede Mod läuft in einer eigenen
//! Lua-5.4-Umgebung ohne Datei-, Netzwerk- oder Systemzugriff (nur table/string/math/utf8),
//! mit Speicherlimit (16 MB) und Zeitlimit pro Aufruf. Fehler schalten nur die betroffene Mod ab.
//!
//! API (siehe LUA_MODS.md):
//!   mod = { name = "...", author = "...", version = "1.0", description = "..." }
//!   jnd.obstacle{ id, w, h, color = {r,g,b}, weight, speed, top }  -- eigenes Hindernis
//!   jnd.modifier{ gravity, jump, speed, lives }                    -- Spielwerte (Faktoren / +Leben)
//!   jnd.mode{ id, name, description, speed, min_delay, max_delay, lives, top }  -- eigener Modus
//!   jnd.log(text)
//!   function on_round_start(info) end           -- info: difficulty, width, height, mode
//!   function on_tick(state) return {spawn = "id" | {"id", ...}} end   -- 60x pro Sekunde
//!   function on_hit(state) end
//! Runden mit aktiven Mods zählen getrennt (wie in der Python-Version) und nicht online.

use mlua::{Function, HookTriggers, Lua, LuaOptions, StdLib, Table, Value, VmState};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct ModObstacle { pub id: String, pub w: i32, pub h: i32, pub color: (i32, i32, i32), pub weight: f64, pub speed: f64, pub top: bool }

#[derive(Clone, Debug)]
pub struct ModModifier { pub gravity: f64, pub jump: f64, pub speed: f64, pub lives: i64 }
impl Default for ModModifier { fn default() -> Self { ModModifier { gravity: 1.0, jump: 1.0, speed: 1.0, lives: 0 } } }

#[derive(Clone, Debug)]
pub struct ModMode { pub id: String, pub name: String, pub description: String, pub speed: f64, pub min_delay: i64, pub max_delay: i64, pub lives: i64, pub top: bool, pub mod_index: usize }

pub struct LoadedMod {
    pub file: String,
    pub name: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub error: Option<String>,
    pub obstacles: Vec<ModObstacle>,
    pub modifier: Option<ModModifier>,
    pub modes: Vec<ModMode>,
    lua: Option<Lua>,
    deadline: Rc<Cell<Instant>>,
    pub log: Vec<String>,
}

/// Was ein on_tick-Aufruf zurückgibt
#[derive(Default, Debug)]
pub struct TickResult { pub spawns: Vec<String> }

const MEMORY_LIMIT: usize = 16 * 1024 * 1024;
const LOAD_BUDGET: Duration = Duration::from_millis(200);
const CALL_BUDGET: Duration = Duration::from_millis(4);

fn num(t: &Table, k: &str, d: f64) -> f64 { t.get::<Option<f64>>(k).ok().flatten().unwrap_or(d) }
fn txt(t: &Table, k: &str, d: &str) -> String { t.get::<Option<String>>(k).ok().flatten().unwrap_or_else(|| d.to_string()) }
fn color(t: &Table, k: &str, d: (i32, i32, i32)) -> (i32, i32, i32) {
    match t.get::<Option<Table>>(k).ok().flatten() {
        Some(c) => {
            let g = |i: i64, d: i32| c.get::<Option<f64>>(i).ok().flatten().map(|v| v.clamp(0.0, 255.0) as i32).unwrap_or(d);
            (g(1, d.0), g(2, d.1), g(3, d.2))
        }
        None => d,
    }
}

/// Sandbox anlegen: nur sichere Bibliotheken, Speicher- und Zeitlimit
fn sandbox(deadline: Rc<Cell<Instant>>) -> mlua::Result<Lua> {
    let lua = Lua::new_with(StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8, LuaOptions::new())?;
    lua.set_memory_limit(MEMORY_LIMIT)?;
    let dl = deadline.clone();
    lua.set_hook(HookTriggers::new().every_nth_instruction(2000), move |_lua, _dbg| {
        if Instant::now() > dl.get() { Err(mlua::Error::runtime("Zeitlimit überschritten (Endlosschleife?)")) } else { Ok(VmState::Continue) }
    });
    let g = lua.globals();
    // gefährliche oder unnötige Funktionen entfernen (Bytecode laden, Dateien, GC-Steuerung)
    for f in ["load", "loadfile", "dofile", "require", "collectgarbage", "rawset", "rawget", "setmetatable", "getmetatable"] { g.set(f, Value::Nil)?; }
    if let Ok(s) = g.get::<Table>("string") { s.set("dump", Value::Nil)?; }
    Ok(lua)
}

impl LoadedMod {
    fn load(path: &Path, index: usize) -> LoadedMod {
        let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let deadline = Rc::new(Cell::new(Instant::now() + LOAD_BUDGET));
        let mut m = LoadedMod {
            name: file.trim_end_matches(".lua").to_string(), file, author: String::new(), version: String::new(), description: String::new(),
            enabled: true, error: None, obstacles: vec![], modifier: None, modes: vec![], lua: None, deadline, log: vec![],
        };
        let src = match std::fs::read_to_string(path) { Ok(s) => s, Err(e) => { m.error = Some(format!("nicht lesbar: {}", e)); return m; } };
        if src.len() > 512 * 1024 { m.error = Some("Datei zu groß (max. 512 KB)".into()); return m; }
        match m.run_file(&src, index) {
            Ok(()) => {}
            Err(e) => { m.error = Some(short_err(&e)); m.lua = None; }
        }
        m
    }

    fn run_file(&mut self, src: &str, index: usize) -> mlua::Result<()> {
        let lua = sandbox(self.deadline.clone())?;
        let reg = lua.create_table()?;
        reg.set("obstacles", lua.create_table()?)?;
        reg.set("modes", lua.create_table()?)?;
        reg.set("log", lua.create_table()?)?;
        lua.set_named_registry_value("jnd_reg", &reg)?;
        let jnd = lua.create_table()?;
        let push = |lua: &Lua, key: &'static str| -> mlua::Result<Function> {
            lua.create_function(move |lua, t: Table| {
                let reg: Table = lua.named_registry_value("jnd_reg")?;
                let list: Table = reg.get(key)?;
                if list.raw_len() < 64 { list.raw_set(list.raw_len() + 1, t)?; }
                Ok(())
            })
        };
        jnd.set("obstacle", push(&lua, "obstacles")?)?;
        jnd.set("mode", push(&lua, "modes")?)?;
        jnd.set("modifier", lua.create_function(|lua, t: Table| {
            let reg: Table = lua.named_registry_value("jnd_reg")?;
            reg.set("modifier", t)
        })?)?;
        jnd.set("log", lua.create_function(|lua, v: Value| {
            let reg: Table = lua.named_registry_value("jnd_reg")?;
            let list: Table = reg.get("log")?;
            let s = match v { Value::String(s) => s.to_str().map(|x| x.to_string()).unwrap_or_default(), other => format!("{:?}", other) };
            if list.raw_len() < 200 { list.raw_set(list.raw_len() + 1, s.chars().take(200).collect::<String>())?; }
            Ok(())
        })?)?;
        lua.globals().set("jnd", jnd)?;
        // print -> Mod-Log
        lua.globals().set("print", lua.globals().get::<Table>("jnd")?.get::<Function>("log")?)?;
        self.deadline.set(Instant::now() + LOAD_BUDGET);
        lua.load(src).set_name(&self.file).set_mode(mlua::ChunkMode::Text).exec()?;
        // Infos einsammeln
        if let Ok(Some(info)) = lua.globals().get::<Option<Table>>("mod") {
            self.name = txt(&info, "name", &self.name).chars().take(40).collect();
            self.author = txt(&info, "author", "").chars().take(40).collect();
            self.version = txt(&info, "version", "").chars().take(16).collect();
            self.description = txt(&info, "description", "").chars().take(160).collect();
        }
        for t in reg.get::<Table>("obstacles")?.sequence_values::<Table>().flatten() {
            let id = txt(&t, "id", "");
            if id.is_empty() { continue; }
            self.obstacles.push(ModObstacle {
                id, w: num(&t, "w", 50.0).clamp(10.0, 200.0) as i32, h: num(&t, "h", 50.0).clamp(10.0, 200.0) as i32,
                color: color(&t, "color", (226, 58, 82)), weight: num(&t, "weight", 10.0).clamp(0.0, 1000.0),
                speed: num(&t, "speed", 1.0).clamp(0.2, 4.0), top: t.get::<Option<bool>>("top").ok().flatten().unwrap_or(false),
            });
        }
        if let Ok(Some(t)) = reg.get::<Option<Table>>("modifier") {
            self.modifier = Some(ModModifier {
                gravity: num(&t, "gravity", 1.0).clamp(0.3, 3.0), jump: num(&t, "jump", 1.0).clamp(0.3, 3.0),
                speed: num(&t, "speed", 1.0).clamp(0.3, 4.0), lives: num(&t, "lives", 0.0).clamp(-2.0, 20.0) as i64,
            });
        }
        for t in reg.get::<Table>("modes")?.sequence_values::<Table>().flatten() {
            let id = txt(&t, "id", "");
            if id.is_empty() { continue; }
            let min_delay = num(&t, "min_delay", 900.0).clamp(80.0, 10000.0) as i64;
            self.modes.push(ModMode {
                name: txt(&t, "name", &id).chars().take(30).collect(), id, description: txt(&t, "description", "").chars().take(120).collect(),
                speed: num(&t, "speed", 8.0).clamp(1.0, 60.0), min_delay, max_delay: (num(&t, "max_delay", 1600.0) as i64).clamp(min_delay, 20000),
                lives: num(&t, "lives", 3.0).clamp(1.0, 99.0) as i64, top: t.get::<Option<bool>>("top").ok().flatten().unwrap_or(false), mod_index: index,
            });
        }
        self.collect_log(&lua);
        self.lua = Some(lua);
        Ok(())
    }

    fn collect_log(&mut self, lua: &Lua) {
        if let Ok(reg) = lua.named_registry_value::<Table>("jnd_reg") {
            if let Ok(list) = reg.get::<Table>("log") {
                for s in list.sequence_values::<String>().flatten() { self.log.push(s); }
                let _ = reg.set("log", lua.create_table().ok());
            }
        }
        if self.log.len() > 50 { let n = self.log.len() - 50; self.log.drain(0..n); }
    }

    /// Callback aufrufen (falls vorhanden). Fehler schalten die Mod ab.
    fn call(&mut self, name: &str, arg: impl FnOnce(&Lua) -> mlua::Result<Table>) -> Option<Value> {
        if !self.enabled || self.error.is_some() { return None; }
        let lua = self.lua.as_ref()?;
        let f: Function = match lua.globals().get::<Option<Function>>(name) { Ok(Some(f)) => f, _ => return None };
        self.deadline.set(Instant::now() + CALL_BUDGET);
        let res = arg(lua).and_then(|a| f.call::<Value>(a));
        let lua2 = self.lua.take();
        if let Some(l) = &lua2 { self.collect_log(l); }
        self.lua = lua2;
        match res {
            Ok(v) => Some(v),
            Err(e) => {
                let msg = format!("{} in {}: {}", self.file, name, short_err(&e));
                eprintln!("[MOD] {}", msg);
                self.error = Some(short_err(&e));
                self.log.push(format!("Fehler: {}", msg));
                None
            }
        }
    }
}

fn short_err(e: &mlua::Error) -> String {
    let s = e.to_string();
    s.lines().next().unwrap_or("Fehler").chars().take(160).collect()
}

#[derive(Default)]
pub struct Mods { pub list: Vec<LoadedMod>, pub dir: PathBuf }

impl Mods {
    /// Alle `mods/*.lua` laden; `enabled` = gespeicherter An/Aus-Zustand je Datei
    pub fn load(dir: &Path, enabled: &serde_json::Map<String, serde_json::Value>) -> Mods {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "lua").unwrap_or(false)).collect()).unwrap_or_default();
        files.sort();
        files.truncate(32);
        let mut list = vec![];
        for (i, p) in files.iter().enumerate() {
            let mut m = LoadedMod::load(p, i);
            m.enabled = enabled.get(&m.file).and_then(|v| v.as_bool()).unwrap_or(true);
            list.push(m);
        }
        Mods { list, dir: dir.to_path_buf() }
    }

    pub fn active(&self) -> impl Iterator<Item = &LoadedMod> { self.list.iter().filter(|m| m.enabled && m.error.is_none()) }
    pub fn any_active(&self) -> bool { self.active().next().is_some() }
    pub fn obstacles(&self) -> Vec<ModObstacle> { self.active().flat_map(|m| m.obstacles.clone()).collect() }
    pub fn modes(&self) -> Vec<ModMode> { self.active().flat_map(|m| m.modes.clone()).collect() }
    /// Alle Modifikatoren zusammen (Faktoren multipliziert, Leben addiert)
    pub fn modifier(&self) -> ModModifier {
        let mut r = ModModifier::default();
        for m in self.active().filter_map(|m| m.modifier.clone()) {
            r.gravity *= m.gravity; r.jump *= m.jump; r.speed *= m.speed; r.lives += m.lives;
        }
        r.gravity = r.gravity.clamp(0.3, 3.0); r.jump = r.jump.clamp(0.3, 3.0); r.speed = r.speed.clamp(0.3, 4.0);
        r
    }

    pub fn on_round_start(&mut self, difficulty: &str, w: i32, h: i32, mode: &str) {
        for m in self.list.iter_mut() {
            m.call("on_round_start", |lua| {
                let t = lua.create_table()?;
                t.set("difficulty", difficulty)?; t.set("width", w)?; t.set("height", h)?; t.set("mode", mode)?;
                Ok(t)
            });
        }
    }

    /// 60x pro Sekunde während der Runde
    pub fn on_tick(&mut self, tick: u64, score: i64, lives: i64, px: i32, py: i32, w: i32, h: i32) -> TickResult {
        let mut out = TickResult::default();
        for m in self.list.iter_mut() {
            let v = m.call("on_tick", |lua| {
                let t = lua.create_table()?;
                t.set("tick", tick)?; t.set("score", score)?; t.set("lives", lives)?; t.set("width", w)?; t.set("height", h)?;
                let p = lua.create_table()?; p.set("x", px)?; p.set("y", py)?; t.set("player", p)?;
                Ok(t)
            });
            if let Some(Value::Table(t)) = v {
                match t.get::<Value>("spawn") {
                    Ok(Value::String(s)) => if let Ok(s) = s.to_str() { out.spawns.push(s.to_string()); },
                    Ok(Value::Table(list)) => for s in list.sequence_values::<String>().flatten().take(8) { out.spawns.push(s); },
                    _ => {}
                }
            }
        }
        out.spawns.truncate(16);
        out
    }

    pub fn on_hit(&mut self, score: i64, lives: i64) {
        for m in self.list.iter_mut() {
            m.call("on_hit", |lua| { let t = lua.create_table()?; t.set("score", score)?; t.set("lives", lives)?; Ok(t) });
        }
    }
}

/// Beispiel-Mod, die das Spiel auf Wunsch anlegt
pub const EXAMPLE_MOD: &str = r#"-- Beispiel-Mod für JumpNDodge (Rust Edition)
-- Mods laufen sicher in einer Sandbox: kein Zugriff auf Dateien, Netzwerk oder System.
-- Runden mit aktiven Mods zählen getrennt und nicht für die Online-Rangliste.

mod = {
  name = "Beispiel: Goldregen",
  author = "Du",
  version = "1.0",
  description = "Kleine goldene Blöcke, etwas leichtere Schwerkraft und ein eigener Modus.",
}

-- Eigenes Hindernis: klein, gelb, etwas schneller; weight = wie oft es statt eines normalen kommt
jnd.obstacle{ id = "gold", w = 30, h = 30, color = {240, 200, 60}, weight = 25, speed = 1.3 }
-- Fällt von oben:
jnd.obstacle{ id = "tropfen", w = 26, h = 40, color = {90, 170, 255}, weight = 0, top = true }

-- Spielwerte: Faktoren (1 = normal) und zusätzliche Leben
jnd.modifier{ gravity = 0.9, jump = 1.0, speed = 1.0, lives = 1 }

-- Eigener Modus (spielbar im Mods-Menü)
jnd.mode{ id = "sprint", name = "Sprint", description = "Schnell, nur ein Leben.",
          speed = 13, min_delay = 350, max_delay = 800, lives = 1, top = true }

function on_round_start(info)
  jnd.log("Runde startet: " .. info.difficulty .. " (" .. info.width .. "x" .. info.height .. ")")
end

-- 60x pro Sekunde. Rückgabe {spawn = "id"} erzeugt sofort ein eigenes Hindernis.
function on_tick(state)
  if state.tick % 300 == 0 and state.tick > 0 then
    return { spawn = "tropfen" }   -- alle 5 Sekunden ein Tropfen von oben
  end
end

function on_hit(state)
  jnd.log("Treffer! Noch " .. state.lives .. " Leben")
end
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, src: &str) { std::fs::write(dir.join(name), src).unwrap(); }

    #[test]
    fn beispiel_laedt_und_laeuft() {
        let d = std::env::temp_dir().join(format!("jnd-mods-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        write(&d, "beispiel.lua", EXAMPLE_MOD);
        let mut m = Mods::load(&d, &serde_json::Map::new());
        assert_eq!(m.list.len(), 1);
        assert!(m.list[0].error.is_none(), "{:?}", m.list[0].error);
        assert_eq!(m.obstacles().len(), 2);
        assert_eq!(m.modes()[0].name, "Sprint");
        assert_eq!(m.modifier().lives, 1);
        m.on_round_start("Mittel", 800, 600, "standard");
        assert!(m.on_tick(299, 0, 3, 50, 500, 800, 600).spawns.is_empty());
        assert_eq!(m.on_tick(300, 0, 3, 50, 500, 800, 600).spawns, vec!["tropfen".to_string()]);
        assert!(m.list[0].log.iter().any(|l| l.contains("Runde startet")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sandbox_ist_dicht() {
        let d = std::env::temp_dir().join(format!("jnd-mods-sb-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        write(&d, "a_io.lua", "io.open('x.txt','w')");
        write(&d, "b_os.lua", "os.execute('echo hi')");
        write(&d, "c_load.lua", "load('return 1')()");
        write(&d, "d_loop.lua", "while true do end");
        write(&d, "e_mem.lua", "local t = {} for i = 1, 1e9 do t[i] = string.rep('x', 1000) end");
        write(&d, "f_tick.lua", "function on_tick(s) while true do end end");
        let mut m = Mods::load(&d, &serde_json::Map::new());
        for i in 0..5 { assert!(m.list[i].error.is_some(), "{} sollte scheitern", m.list[i].file); }
        assert!(m.list[5].error.is_none());
        let t0 = Instant::now();
        m.on_tick(1, 0, 3, 0, 0, 800, 600);
        assert!(t0.elapsed() < Duration::from_millis(200), "Endlosschleife wird abgebrochen");
        assert!(m.list[5].error.is_some(), "Mod mit Endlosschleife wird abgeschaltet");
        let _ = std::fs::remove_dir_all(&d);
    }
}
