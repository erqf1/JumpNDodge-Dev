//! Hilfen, um Python-Code mit dict/list-Werten (JSON) möglichst direkt zu übertragen.

use rand::Rng;
use serde_json::{Map, Value};

pub type Obj = Map<String, Value>;

/// `d.get(k, default)` für JSON-Objekte - Typumwandlung wie Python es "meint".
pub trait PyGet {
    fn gv(&self, k: &str) -> Option<&Value>;
    fn gb(&self, k: &str, d: bool) -> bool { self.gv(k).map(truthy_or(d)).unwrap_or(d) }
    fn gf(&self, k: &str, d: f64) -> f64 { self.gv(k).and_then(as_f).unwrap_or(d) }
    fn gi(&self, k: &str, d: i64) -> i64 { self.gv(k).and_then(as_i).unwrap_or(d) }
    fn gs(&self, k: &str, d: &str) -> String {
        match self.gv(k) { Some(Value::String(s)) => s.clone(), Some(Value::Null) | None => d.to_string(), Some(v) => py_str(v) }
    }
    /// Wie gs, aber None bleibt None
    fn gos(&self, k: &str) -> Option<String> {
        match self.gv(k) { Some(Value::String(s)) => Some(s.clone()), Some(Value::Null) | None => None, Some(v) => Some(py_str(v)) }
    }
    fn garr(&self, k: &str) -> Vec<Value> { match self.gv(k) { Some(Value::Array(a)) => a.clone(), _ => vec![] } }
    fn gstrs(&self, k: &str) -> Vec<String> { self.garr(k).iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect() }
    fn gobj(&self, k: &str) -> Obj { match self.gv(k) { Some(Value::Object(o)) => o.clone(), _ => Obj::new() } }
    fn has(&self, k: &str) -> bool { self.gv(k).is_some() }
    /// Python-Wahrheitswert von d.get(k)
    fn truthy(&self, k: &str) -> bool { self.gv(k).map(is_truthy).unwrap_or(false) }
}

impl PyGet for Obj { fn gv(&self, k: &str) -> Option<&Value> { self.get(k) } }
impl PyGet for Value { fn gv(&self, k: &str) -> Option<&Value> { self.as_object().and_then(|o| o.get(k)) } }

fn truthy_or(_d: bool) -> impl Fn(&Value) -> bool { is_truthy }

pub fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}
pub fn as_f(v: &Value) -> Option<f64> {
    match v { Value::Number(n) => n.as_f64(), Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }), Value::String(s) => s.parse().ok(), _ => None }
}
pub fn as_i(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::Bool(b) => Some(*b as i64),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}
/// str(v) wie Python
pub fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        Value::Bool(b) => if *b { "True".into() } else { "False".into() },
        Value::Number(n) => {
            if n.is_f64() { py_float(n.as_f64().unwrap()) } else { n.to_string() }
        }
        _ => v.to_string(),
    }
}
/// str(float) wie Python (1.0 statt 1)
pub fn py_float(f: f64) -> String {
    if f.is_finite() && f == f.trunc() && f.abs() < 1e16 { format!("{:.1}", f) } else { format!("{}", f) }
}

/// JSON-Zahl, die als Python-int gespeichert wird
pub fn ji(v: i64) -> Value { Value::from(v) }
/// JSON-Zahl, die als Python-float gespeichert wird
pub fn jf(v: f64) -> Value { serde_json::Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null) }

pub fn obj_of(v: &Value) -> Obj { v.as_object().cloned().unwrap_or_default() }
pub fn strs(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}
pub fn ints(v: &Value) -> Vec<i64> { v.as_array().map(|a| a.iter().filter_map(as_i).collect()).unwrap_or_default() }

/// {**a, **b} wie in Python
pub fn merged(a: &Obj, b: &Obj) -> Obj {
    let mut o = a.clone();
    for (k, v) in b { o.insert(k.clone(), v.clone()); }
    o
}

// ---- random wie Python ----
pub fn randint(a: i64, b: i64) -> i64 { if b < a { a } else { rand::thread_rng().gen_range(a..=b) } }
pub fn random() -> f64 { rand::thread_rng().gen::<f64>() }
pub fn uniform(a: f64, b: f64) -> f64 { a + (b - a) * random() }
pub fn choice<T: Clone>(v: &[T]) -> T { v[rand::thread_rng().gen_range(0..v.len())].clone() }
/// random.choices(items, weights=w, k=1)[0]
pub fn weighted_choice<T: Clone>(items: &[T], weights: &[f64]) -> T {
    let total: f64 = weights.iter().sum();
    let mut r = random() * total;
    for (i, w) in weights.iter().enumerate() {
        if r < *w { return items[i].clone(); }
        r -= w;
    }
    items[items.len() - 1].clone()
}
pub fn shuffle<T>(v: &mut [T]) { use rand::seq::SliceRandom; v.shuffle(&mut rand::thread_rng()); }
pub fn sample<T: Clone>(v: &[T], k: usize) -> Vec<T> {
    use rand::seq::SliceRandom;
    v.choose_multiple(&mut rand::thread_rng(), k.min(v.len())).cloned().collect()
}

/// colorsys.hsv_to_rgb
pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    if s == 0.0 { return (v, v, v); }
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match (i as i64).rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

/// Python round() (Banker's rounding)
pub fn py_round(x: f64) -> i64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 { (2.0 * (x / 2.0).round()) as i64 } else { r as i64 }
}

/// Python // für ints (abrunden)
pub fn fdiv(a: i32, b: i32) -> i32 { a.div_euclid(b) - if (a.rem_euclid(b) != 0) && (b < 0) { 1 } else { 0 } }

/// "text".title() wie Python
pub fn title_case(s: &str) -> String {
    let mut out = String::new();
    let mut prev_alpha = false;
    for c in s.chars() {
        if c.is_alphabetic() {
            if prev_alpha { out.extend(c.to_lowercase()); } else { out.extend(c.to_uppercase()); }
            prev_alpha = true;
        } else {
            out.push(c);
            prev_alpha = false;
        }
    }
    out
}

/// URL-Kodierung wie urllib.parse.quote (safe='/')
pub fn quote(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || "_.-~/".contains(c) { out.push(c); } else { out.push_str(&format!("%{:02X}", b)); }
    }
    out
}

/// Python str[:n] für Zeichen
pub fn take_chars(s: &str, n: usize) -> String { s.chars().take(n).collect() }
pub fn char_len(s: &str) -> usize { s.chars().count() }
