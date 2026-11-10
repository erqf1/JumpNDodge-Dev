//! pygame-Tastencodes (SDL2-Keycodes) - gleiche Zahlen wie in pygame, damit gespeicherte
//! Tastenbelegungen aus der Python-Version weiter stimmen.

pub const K_BACKSPACE: i32 = 8;
pub const K_TAB: i32 = 9;
pub const K_RETURN: i32 = 13;
pub const K_ESCAPE: i32 = 27;
pub const K_SPACE: i32 = 32;
pub const K_DELETE: i32 = 127;
pub const K_0: i32 = 48;
pub const K_1: i32 = 49;
pub const K_2: i32 = 50;
pub const K_3: i32 = 51;
pub const K_4: i32 = 52;
pub const K_5: i32 = 53;
pub const K_6: i32 = 54;
pub const K_7: i32 = 55;
pub const K_8: i32 = 56;
pub const K_9: i32 = 57;
pub const K_a: i32 = 97;
pub const K_b: i32 = 98;
pub const K_c: i32 = 99;
pub const K_d: i32 = 100;
pub const K_e: i32 = 101;
pub const K_f: i32 = 102;
pub const K_g: i32 = 103;
pub const K_h: i32 = 104;
pub const K_i: i32 = 105;
pub const K_j: i32 = 106;
pub const K_k: i32 = 107;
pub const K_l: i32 = 108;
pub const K_m: i32 = 109;
pub const K_n: i32 = 110;
pub const K_o: i32 = 111;
pub const K_p: i32 = 112;
pub const K_q: i32 = 113;
pub const K_r: i32 = 114;
pub const K_s: i32 = 115;
pub const K_t: i32 = 116;
pub const K_u: i32 = 117;
pub const K_v: i32 = 118;
pub const K_w: i32 = 119;
pub const K_x: i32 = 120;
pub const K_y: i32 = 121;
pub const K_z: i32 = 122;
const S: i32 = 1 << 30;
pub const K_CAPSLOCK: i32 = 57 | S;
pub const K_F1: i32 = 58 | S;
pub const K_F2: i32 = 59 | S;
pub const K_F3: i32 = 60 | S;
pub const K_F4: i32 = 61 | S;
pub const K_F5: i32 = 62 | S;
pub const K_F6: i32 = 63 | S;
pub const K_F7: i32 = 64 | S;
pub const K_F8: i32 = 65 | S;
pub const K_F9: i32 = 66 | S;
pub const K_F10: i32 = 67 | S;
pub const K_F11: i32 = 68 | S;
pub const K_F12: i32 = 69 | S;
pub const K_PRINT: i32 = 70 | S;
pub const K_SCROLLLOCK: i32 = 71 | S;
pub const K_PAUSE: i32 = 72 | S;
pub const K_INSERT: i32 = 73 | S;
pub const K_HOME: i32 = 74 | S;
pub const K_PAGEUP: i32 = 75 | S;
pub const K_END: i32 = 77 | S;
pub const K_PAGEDOWN: i32 = 78 | S;
pub const K_RIGHT: i32 = 79 | S;
pub const K_LEFT: i32 = 80 | S;
pub const K_DOWN: i32 = 81 | S;
pub const K_UP: i32 = 82 | S;
pub const K_NUMLOCK: i32 = 83 | S;
pub const K_KP_DIVIDE: i32 = 84 | S;
pub const K_KP_MULTIPLY: i32 = 85 | S;
pub const K_KP_MINUS: i32 = 86 | S;
pub const K_KP_PLUS: i32 = 87 | S;
pub const K_KP_ENTER: i32 = 88 | S;
pub const K_KP1: i32 = 89 | S;
pub const K_KP2: i32 = 90 | S;
pub const K_KP3: i32 = 91 | S;
pub const K_KP4: i32 = 92 | S;
pub const K_KP5: i32 = 93 | S;
pub const K_KP6: i32 = 94 | S;
pub const K_KP7: i32 = 95 | S;
pub const K_KP8: i32 = 96 | S;
pub const K_KP9: i32 = 97 | S;
pub const K_KP0: i32 = 98 | S;
pub const K_KP_PERIOD: i32 = 99 | S;
pub const K_MENU: i32 = 101 | S;
pub const K_LCTRL: i32 = 224 | S;
pub const K_LSHIFT: i32 = 225 | S;
pub const K_LALT: i32 = 226 | S;
pub const K_LGUI: i32 = 227 | S;
pub const K_RCTRL: i32 = 228 | S;
pub const K_RSHIFT: i32 = 229 | S;
pub const K_RALT: i32 = 230 | S;
pub const K_RGUI: i32 = 231 | S;

pub const KMOD_LSHIFT: u16 = 0x0001;
pub const KMOD_RSHIFT: u16 = 0x0002;
pub const KMOD_SHIFT: u16 = 0x0003;
pub const KMOD_LCTRL: u16 = 0x0040;
pub const KMOD_RCTRL: u16 = 0x0080;
pub const KMOD_CTRL: u16 = 0x00C0;
pub const KMOD_LALT: u16 = 0x0100;
pub const KMOD_RALT: u16 = 0x0200;
pub const KMOD_ALT: u16 = 0x0300;
pub const KMOD_GUI: u16 = 0x0C00;
pub const KMOD_CAPS: u16 = 0x2000;

/// pygame.key.name(k)
pub fn key_name(k: i32) -> String {
    let n = match k {
        K_BACKSPACE => "backspace",
        K_TAB => "tab",
        K_RETURN => "return",
        K_ESCAPE => "escape",
        K_SPACE => "space",
        K_DELETE => "delete",
        K_CAPSLOCK => "caps lock",
        K_F1 => "f1", K_F2 => "f2", K_F3 => "f3", K_F4 => "f4", K_F5 => "f5", K_F6 => "f6",
        K_F7 => "f7", K_F8 => "f8", K_F9 => "f9", K_F10 => "f10", K_F11 => "f11", K_F12 => "f12",
        K_PRINT => "print screen",
        K_SCROLLLOCK => "scroll lock",
        K_PAUSE => "pause",
        K_INSERT => "insert",
        K_HOME => "home",
        K_PAGEUP => "page up",
        K_END => "end",
        K_PAGEDOWN => "page down",
        K_RIGHT => "right",
        K_LEFT => "left",
        K_DOWN => "down",
        K_UP => "up",
        K_NUMLOCK => "numlock",
        K_KP_DIVIDE => "[/]",
        K_KP_MULTIPLY => "[*]",
        K_KP_MINUS => "[-]",
        K_KP_PLUS => "[+]",
        K_KP_ENTER => "enter",
        K_KP1 => "[1]", K_KP2 => "[2]", K_KP3 => "[3]", K_KP4 => "[4]", K_KP5 => "[5]",
        K_KP6 => "[6]", K_KP7 => "[7]", K_KP8 => "[8]", K_KP9 => "[9]", K_KP0 => "[0]",
        K_KP_PERIOD => "[.]",
        K_MENU => "menu",
        K_LCTRL => "left ctrl",
        K_LSHIFT => "left shift",
        K_LALT => "left alt",
        K_LGUI => "left meta",
        K_RCTRL => "right ctrl",
        K_RSHIFT => "right shift",
        K_RALT => "right alt",
        K_RGUI => "right meta",
        _ => {
            if (1..0x110000).contains(&k) && k & S == 0 {
                if let Some(c) = char::from_u32(k as u32) {
                    return c.to_string();
                }
            }
            return "unknown key".into();
        }
    };
    n.to_string()
}

/// Zuordnung von winit-Tasten auf pygame-Codes
pub fn from_winit(ev: &winit::event::KeyEvent, unshifted: Option<&str>) -> i32 {
    use winit::keyboard::{Key, KeyCode, KeyLocation, NamedKey, PhysicalKey};
    // Ziffernblock nach physischer Taste
    if let PhysicalKey::Code(code) = ev.physical_key {
        let kp = match code {
            KeyCode::Numpad0 => Some(K_KP0), KeyCode::Numpad1 => Some(K_KP1), KeyCode::Numpad2 => Some(K_KP2),
            KeyCode::Numpad3 => Some(K_KP3), KeyCode::Numpad4 => Some(K_KP4), KeyCode::Numpad5 => Some(K_KP5),
            KeyCode::Numpad6 => Some(K_KP6), KeyCode::Numpad7 => Some(K_KP7), KeyCode::Numpad8 => Some(K_KP8),
            KeyCode::Numpad9 => Some(K_KP9), KeyCode::NumpadDecimal => Some(K_KP_PERIOD),
            KeyCode::NumpadDivide => Some(K_KP_DIVIDE), KeyCode::NumpadMultiply => Some(K_KP_MULTIPLY),
            KeyCode::NumpadSubtract => Some(K_KP_MINUS), KeyCode::NumpadAdd => Some(K_KP_PLUS),
            KeyCode::NumpadEnter => Some(K_KP_ENTER),
            _ => None,
        };
        if let Some(k) = kp { return k; }
    }
    let right = ev.location == KeyLocation::Right;
    match &ev.logical_key {
        Key::Named(n) => match n {
            NamedKey::Backspace => K_BACKSPACE,
            NamedKey::Tab => K_TAB,
            NamedKey::Enter => K_RETURN,
            NamedKey::Escape => K_ESCAPE,
            NamedKey::Space => K_SPACE,
            NamedKey::Delete => K_DELETE,
            NamedKey::CapsLock => K_CAPSLOCK,
            NamedKey::F1 => K_F1, NamedKey::F2 => K_F2, NamedKey::F3 => K_F3, NamedKey::F4 => K_F4,
            NamedKey::F5 => K_F5, NamedKey::F6 => K_F6, NamedKey::F7 => K_F7, NamedKey::F8 => K_F8,
            NamedKey::F9 => K_F9, NamedKey::F10 => K_F10, NamedKey::F11 => K_F11, NamedKey::F12 => K_F12,
            NamedKey::PrintScreen => K_PRINT,
            NamedKey::ScrollLock => K_SCROLLLOCK,
            NamedKey::Pause => K_PAUSE,
            NamedKey::Insert => K_INSERT,
            NamedKey::Home => K_HOME,
            NamedKey::PageUp => K_PAGEUP,
            NamedKey::End => K_END,
            NamedKey::PageDown => K_PAGEDOWN,
            NamedKey::ArrowRight => K_RIGHT,
            NamedKey::ArrowLeft => K_LEFT,
            NamedKey::ArrowDown => K_DOWN,
            NamedKey::ArrowUp => K_UP,
            NamedKey::NumLock => K_NUMLOCK,
            NamedKey::ContextMenu => K_MENU,
            NamedKey::Control => if right { K_RCTRL } else { K_LCTRL },
            NamedKey::Shift => if right { K_RSHIFT } else { K_LSHIFT },
            NamedKey::Alt => if right { K_RALT } else { K_LALT },
            NamedKey::AltGraph => K_RALT,
            NamedKey::Super | NamedKey::Meta => if right { K_RGUI } else { K_LGUI },
            _ => 0,
        },
        Key::Character(s) => {
            let s = unshifted.unwrap_or(s.as_str());
            match s.chars().next() {
                Some(c) => c.to_lowercase().next().unwrap_or(c) as i32,
                None => 0,
            }
        }
        _ => 0,
    }
}
