//! pygame.display, pygame.event, pygame.key, pygame.mouse, pygame.time - mit winit + softbuffer.
//!
//! Das Spiel ist wie in Python aufgebaut: viele eigene while-Schleifen, die am Ende
//! `flip()` aufrufen. winit wird deshalb im "pump"-Modus betrieben: Jeder Aufruf von
//! `pump()` holt die anstehenden Fenster-Ereignisse ab und legt sie in eine Warteschlange,
//! die `event_get()` wie pygame.event.get() leert.

use super::keys::*;
use super::surface::Surface;
use std::collections::{HashSet, VecDeque};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::window::{Fullscreen, Window, WindowId};

#[derive(Clone, Debug)]
pub enum Event {
    Quit,
    KeyDown { key: i32, unicode: String, mods: u16 },
    KeyUp { key: i32, mods: u16 },
    MouseButtonDown { button: u8, pos: (i32, i32) },
    MouseButtonUp { button: u8, pos: (i32, i32) },
    MouseMotion { pos: (i32, i32), rel: (i32, i32) },
    MouseWheel { x: i32, y: i32 },
    VideoResize { w: i32, h: i32 },
    TextInput { text: String },
    FocusLost,
    FocusGained,
}

struct App {
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    _ctx: Option<softbuffer::Context<Rc<Window>>>,
    events: VecDeque<Event>,
    title: String,
    init_size: (u32, u32),
    resizable: bool,
    visible: bool,
    mouse_phys: (f64, f64),
    keys_down: HashSet<i32>,
    mods: u16,
    mouse_buttons: [bool; 3],
    focused: bool,
    icon: Option<winit::window::Icon>,
}

impl App {
    fn scale(&self) -> f64 { self.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0) }
    fn logical_mouse(&self) -> (i32, i32) {
        let s = self.scale();
        ((self.mouse_phys.0 / s) as i32, (self.mouse_phys.1 / s) as i32)
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        let mut attrs = Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(LogicalSize::new(self.init_size.0, self.init_size.1))
            .with_resizable(self.resizable)
            .with_visible(self.visible)
            .with_min_inner_size(LogicalSize::new(200, 150));
        if let Some(ic) = self.icon.clone() { attrs = attrs.with_window_icon(Some(ic)); }
        let w = Rc::new(el.create_window(attrs).expect("Fenster konnte nicht erstellt werden"));
        let ctx = softbuffer::Context::new(w.clone()).expect("softbuffer");
        let surf = softbuffer::Surface::new(&ctx, w.clone()).expect("softbuffer surface");
        self.window = Some(w);
        self._ctx = Some(ctx);
        self.surface = Some(surf);
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => self.events.push_back(Event::Quit),
            WindowEvent::Resized(ps) => {
                let s = self.scale();
                self.events.push_back(Event::VideoResize { w: (ps.width as f64 / s) as i32, h: (ps.height as f64 / s) as i32 });
            }
            WindowEvent::Focused(f) => {
                self.focused = f;
                if !f {
                    self.keys_down.clear();
                    self.mods = 0;
                    self.events.push_back(Event::FocusLost);
                } else {
                    self.events.push_back(Event::FocusGained);
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                let st = m.state();
                let mut v = 0u16;
                if st.shift_key() { v |= KMOD_LSHIFT; }
                if st.control_key() { v |= KMOD_LCTRL; }
                if st.alt_key() { v |= KMOD_LALT; }
                if st.super_key() { v |= KMOD_GUI; }
                self.mods = v;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
                let unshifted: Option<String> = {
                    use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
                    match event.key_without_modifiers() {
                        winit::keyboard::Key::Character(s) => Some(s.to_string()),
                        _ => None,
                    }
                };
                #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
                let unshifted: Option<String> = None;
                let key = from_winit(&event, unshifted.as_deref());
                match event.state {
                    ElementState::Pressed => {
                        self.keys_down.insert(key);
                        let unicode = event.text.as_ref().map(|t| t.to_string()).unwrap_or_default();
                        let unicode: String = unicode.chars().filter(|c| !c.is_control() || *c == '\r' || *c == '\t' || *c == '\u{8}' || *c == '\u{1b}').collect();
                        if !event.repeat || true {
                            self.events.push_back(Event::KeyDown { key, unicode: unicode.clone(), mods: self.mods });
                        }
                        let printable: String = unicode.chars().filter(|c| !c.is_control()).collect();
                        if !printable.is_empty() && self.mods & KMOD_CTRL == 0 {
                            self.events.push_back(Event::TextInput { text: printable });
                        }
                    }
                    ElementState::Released => {
                        self.keys_down.remove(&key);
                        self.events.push_back(Event::KeyUp { key, mods: self.mods });
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let old = self.logical_mouse();
                self.mouse_phys = (position.x, position.y);
                let new = self.logical_mouse();
                self.events.push_back(Event::MouseMotion { pos: new, rel: (new.0 - old.0, new.1 - old.1) });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button { MouseButton::Left => 1, MouseButton::Middle => 2, MouseButton::Right => 3, MouseButton::Back => 6, MouseButton::Forward => 7, _ => 0 };
                if b == 0 { return; }
                let pos = self.logical_mouse();
                if (1..=3).contains(&b) { self.mouse_buttons[(b - 1) as usize] = state == ElementState::Pressed; }
                match state {
                    ElementState::Pressed => self.events.push_back(Event::MouseButtonDown { button: b, pos }),
                    ElementState::Released => self.events.push_back(Event::MouseButtonUp { button: b, pos }),
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x.round() as i32, y.round() as i32),
                    MouseScrollDelta::PixelDelta(p) => ((p.x / 40.0).round() as i32, (p.y / 40.0).round() as i32),
                };
                if dx == 0 && dy == 0 { return; }
                let pos = self.logical_mouse();
                self.events.push_back(Event::MouseWheel { x: dx, y: dy });
                // pygame 2 meldet das Mausrad zusätzlich als Taste 4 (hoch) / 5 (runter)
                let b = if dy > 0 { 4 } else { 5 };
                if dy != 0 {
                    self.events.push_back(Event::MouseButtonDown { button: b, pos });
                    self.events.push_back(Event::MouseButtonUp { button: b, pos });
                }
            }
            _ => {}
        }
    }
}

/// Controller (Xbox, PlayStation, ...): werden auf Tasten abgebildet, damit alle Menüs und
/// Spielmodi ohne Änderung funktionieren. Controller 1 = W/A/S/D + Leertaste (Einzelspieler,
/// Menüs, P1), Controller 2 = Pfeiltasten (P2 im lokalen 2-Spieler-Modus).
struct Pads {
    gilrs: gilrs::Gilrs,
    order: Vec<gilrs::GamepadId>,
}

fn pad_button_key(pad: usize, b: gilrs::Button) -> Option<i32> {
    use gilrs::Button::*;
    if pad == 0 {
        Some(match b {
            South => K_SPACE, East => K_ESCAPE, West | LeftTrigger => K_q, North | RightTrigger => K_e,
            Start => K_ESCAPE, Select => K_m,
            DPadUp => K_w, DPadDown => K_s, DPadLeft => K_a, DPadRight => K_d,
            _ => return None,
        })
    } else {
        Some(match b {
            South | DPadUp => K_UP, DPadDown => K_DOWN, DPadLeft => K_LEFT, DPadRight => K_RIGHT,
            Start | East => K_ESCAPE,
            _ => return None,
        })
    }
}

pub struct Display {
    el: EventLoop<()>,
    app: App,
    pads: Option<Pads>,
    /// gedrückte (virtuelle) Tasten je Controller und Quelle (0 = Knopf, 1 = Stick)
    pad_held: HashSet<(usize, i32, u8)>,
    /// Meldungen für das Spiel ("Controller verbunden" ...)
    pub pad_messages: Vec<String>,
    start: Instant,
    pub hidden: bool,
    fullscreen: bool,
    present_buf_size: (u32, u32),
}

impl Display {
    pub fn new(title: &str, w: u32, h: u32, resizable: bool, icon_rgba: Option<(Vec<u8>, u32, u32)>) -> Display {
        let hidden = std::env::var("JND_HIDDEN").map(|v| v == "1").unwrap_or(false);
        let el = EventLoop::new().expect("EventLoop");
        let icon = icon_rgba.and_then(|(d, w, h)| winit::window::Icon::from_rgba(d, w, h).ok());
        let mut d = Display {
            el,
            app: App {
                window: None, surface: None, _ctx: None, events: VecDeque::new(), title: title.into(),
                init_size: (w, h), resizable, visible: !hidden, mouse_phys: (0.0, 0.0),
                keys_down: HashSet::new(), mods: 0, mouse_buttons: [false; 3], focused: true, icon,
            },
            start: Instant::now(),
            hidden,
            fullscreen: false,
            present_buf_size: (0, 0),
            pads: None,
            pad_held: HashSet::new(),
            pad_messages: Vec::new(),
        };
        if std::env::var("JND_NO_GAMEPAD").is_err() {
            match gilrs::Gilrs::new() {
                Ok(g) => {
                    let mut p = Pads { gilrs: g, order: Vec::new() };
                    let ids: Vec<gilrs::GamepadId> = p.gilrs.gamepads().map(|(id, _)| id).collect();
                    p.order.extend(ids);
                    d.pads = Some(p);
                }
                Err(e) => eprintln!("[INFO] Controller nicht verfügbar: {}", e),
            }
        }
        for _ in 0..50 {
            d.pump();
            if d.app.window.is_some() { break; }
            std::thread::sleep(Duration::from_millis(5));
        }
        // Beim Start entstandene Resize-Ereignisse verwerfen
        d.app.events.clear();
        d
    }

    /// Holt Fenster-Ereignisse ab (ohne zu blockieren)
    pub fn pump(&mut self) {
        let _ = self.el.pump_app_events(Some(Duration::ZERO), &mut self.app);
        self.pump_pads();
    }

    fn pad_key(&mut self, pad: usize, key: i32, src: u8, down: bool) {
        let held = &mut self.pad_held;
        let was_any = held.iter().any(|h| h.1 == key);
        if down { held.insert((pad, key, src)); } else { held.remove(&(pad, key, src)); }
        let now_any = held.iter().any(|h| h.1 == key);
        if now_any && !was_any {
            self.app.keys_down.insert(key);
            self.app.events.push_back(Event::KeyDown { key, unicode: String::new(), mods: 0 });
        } else if !now_any && was_any {
            self.app.keys_down.remove(&key);
            self.app.events.push_back(Event::KeyUp { key, mods: 0 });
        }
    }

    fn pump_pads(&mut self) {
        loop {
            let Some(p) = self.pads.as_mut() else { return; };
            let Some(ev) = p.gilrs.next_event() else { return; };
            let id = ev.id;
            if !p.order.contains(&id) { p.order.push(id); }
            let pad = p.order.iter().position(|x| *x == id).unwrap_or(0);
            match ev.event {
                gilrs::EventType::Connected => {
                    let name = p.gilrs.gamepad(id).name().to_string();
                    self.pad_messages.push(format!("Controller {} verbunden: {}", pad + 1, name));
                }
                gilrs::EventType::Disconnected => {
                    let keys: Vec<(usize, i32, u8)> = self.pad_held.iter().filter(|h| h.0 == pad).copied().collect();
                    for (pd, k, src) in keys { self.pad_key(pd, k, src, false); }
                    self.pad_messages.push(format!("Controller {} getrennt", pad + 1));
                }
                gilrs::EventType::ButtonPressed(b, _) => { if let Some(k) = pad_button_key(pad, b) { self.pad_key(pad, k, 0, true); } }
                gilrs::EventType::ButtonReleased(b, _) => { if let Some(k) = pad_button_key(pad, b) { self.pad_key(pad, k, 0, false); } }
                gilrs::EventType::AxisChanged(axis, v, _) => {
                    // Linker Stick wie das Steuerkreuz (mit Hysterese)
                    let (neg, pos) = match (axis, pad) {
                        (gilrs::Axis::LeftStickX, 0) => (K_a, K_d),
                        (gilrs::Axis::LeftStickY, 0) => (K_s, K_w),
                        (gilrs::Axis::LeftStickX, _) => (K_LEFT, K_RIGHT),
                        (gilrs::Axis::LeftStickY, _) => (K_DOWN, K_UP),
                        _ => continue,
                    };
                    let held = |k: i32| self.pad_held.contains(&(pad, k, 1));
                    let (hn, hp) = (held(neg), held(pos));
                    if v < -0.55 && !hn { self.pad_key(pad, neg, 1, true); }
                    if v > -0.35 && hn { self.pad_key(pad, neg, 1, false); }
                    if v > 0.55 && !hp { self.pad_key(pad, pos, 1, true); }
                    if v < 0.35 && hp { self.pad_key(pad, pos, 1, false); }
                }
                _ => {}
            }
        }
    }

    /// pygame.event.get()
    pub fn event_get(&mut self) -> Vec<Event> {
        self.pump();
        self.app.events.drain(..).collect()
    }
    /// Ereignisse wieder vorne einreihen (pygame.event.post)
    pub fn event_post(&mut self, e: Event) { self.app.events.push_back(e); }
    pub fn event_clear(&mut self) { self.pump(); self.app.events.clear(); }

    /// pygame.key.get_pressed()[k]
    pub fn key_pressed(&self, k: i32) -> bool { self.app.keys_down.contains(&k) }
    pub fn keys_down(&self) -> &HashSet<i32> { &self.app.keys_down }
    /// pygame.key.get_mods()
    pub fn get_mods(&self) -> u16 { self.app.mods }
    /// pygame.mouse.get_pos() (logische Fensterkoordinaten)
    pub fn mouse_pos(&self) -> (i32, i32) { self.app.logical_mouse() }
    /// pygame.mouse.get_pressed()
    pub fn mouse_pressed(&self) -> (bool, bool, bool) { (self.app.mouse_buttons[0], self.app.mouse_buttons[1], self.app.mouse_buttons[2]) }
    pub fn focused(&self) -> bool { self.app.focused }

    /// pygame.time.get_ticks()
    pub fn get_ticks(&self) -> i64 { self.start.elapsed().as_millis() as i64 }

    /// Fenstergröße in logischen Pixeln (wie pygame display_surface.get_size())
    pub fn size(&self) -> (i32, i32) {
        match &self.app.window {
            Some(w) => {
                let ps = w.inner_size();
                let s = w.scale_factor();
                (((ps.width as f64) / s).round() as i32, ((ps.height as f64) / s).round() as i32)
            }
            None => (self.app.init_size.0 as i32, self.app.init_size.1 as i32),
        }
    }

    /// Echte Pixelgröße des Fensterinhalts
    pub fn physical_size(&self) -> (i32, i32) {
        match &self.app.window {
            Some(w) => { let ps = w.inner_size(); (ps.width as i32, ps.height as i32) }
            None => (self.app.init_size.0 as i32, self.app.init_size.1 as i32),
        }
    }
    /// Skalierung des Betriebssystems (Windows 125 % = 1.25)
    pub fn os_scale(&self) -> f64 { self.app.scale() }
    /// Bildwiederholrate des aktuellen Monitors in Hz (falls bekannt)
    pub fn refresh_rate(&self) -> Option<f64> {
        let w = self.app.window.as_ref()?;
        let m = w.current_monitor()?;
        m.refresh_rate_millihertz().map(|mhz| mhz as f64 / 1000.0).filter(|hz| *hz >= 20.0 && *hz <= 1000.0)
    }

    /// Nur für den versteckten Selbsttest: Controller-Knopf wie ein echtes Ereignis einspeisen
    pub fn test_pad_button(&mut self, pad: usize, name: &str, down: bool) {
        use gilrs::Button::*;
        let b = match name { "a" => South, "b" => East, "x" => West, "y" => North, "start" => Start, "select" => Select,
            "up" => DPadUp, "down" => DPadDown, "left" => DPadLeft, "right" => DPadRight, "lb" => LeftTrigger, "rb" => RightTrigger, _ => return };
        if let Some(k) = pad_button_key(pad, b) { self.pad_key(pad, k, 0, down); }
    }
    /// Nur für den versteckten Selbsttest: linken Stick einspeisen (x/y, -1..1)
    pub fn test_pad_stick(&mut self, pad: usize, x: f32) {
        let (neg, pos) = if pad == 0 { (K_a, K_d) } else { (K_LEFT, K_RIGHT) };
        self.pad_key(pad, neg, 1, x < -0.55);
        self.pad_key(pad, pos, 1, x > 0.55);
    }

    pub fn set_title(&mut self, t: &str) { if let Some(w) = &self.app.window { w.set_title(t); } }

    /// pygame.display.set_mode((w, h), flags)
    pub fn set_mode(&mut self, w: i32, h: i32, resizable: bool, fullscreen: bool) {
        self.app.resizable = resizable;
        if let Some(win) = &self.app.window {
            if fullscreen {
                if !self.fullscreen {
                    win.set_fullscreen(Some(Fullscreen::Borderless(None)));
                }
            } else {
                if self.fullscreen {
                    win.set_fullscreen(None);
                }
                win.set_resizable(resizable);
                let cur = self.size();
                if cur != (w, h) {
                    let _ = win.request_inner_size(LogicalSize::new(w.max(1) as u32, h.max(1) as u32));
                }
            }
        }
        self.fullscreen = fullscreen;
        self.pump();
    }

    /// pygame.display.Info(): Bildschirmgröße (logisch)
    pub fn screen_size(&self) -> (i32, i32) {
        if let Some(w) = &self.app.window {
            if let Some(m) = w.current_monitor() {
                let ps = m.size();
                let s = m.scale_factor();
                return (((ps.width as f64) / s) as i32, ((ps.height as f64) / s) as i32);
            }
        }
        (1920, 1080)
    }

    /// Zeigt eine Fläche (logische Größe) im Fenster an - skaliert auf die echte Pixelgröße.
    pub fn present(&mut self, surf: &Surface) {
        let Some(win) = self.app.window.clone() else { return; };
        let ps: PhysicalSize<u32> = win.inner_size();
        let (pw, ph) = (ps.width.max(1), ps.height.max(1));
        let Some(sb) = self.app.surface.as_mut() else { return; };
        if self.present_buf_size != (pw, ph) {
            if sb.resize(NonZeroU32::new(pw).unwrap(), NonZeroU32::new(ph).unwrap()).is_err() { return; }
            self.present_buf_size = (pw, ph);
        }
        let Ok(mut buf) = sb.buffer_mut() else { return; };
        surf.with_pixmap(|pm| {
            let (sw, sh) = (pm.width(), pm.height());
            let src = pm.pixels();
            let near = sw.abs_diff(pw) <= 3 && sh.abs_diff(ph) <= 3;
            if near {
                // Gleiche Auflösung (Normalfall): 1:1 kopieren, ggf. 1-3 Pixel Rand schwarz
                let cw = sw.min(pw) as usize;
                for y in 0..ph as usize {
                    let drow = &mut buf[y * pw as usize..(y + 1) * pw as usize];
                    if y < sh as usize {
                        let srow = &src[y * sw as usize..y * sw as usize + cw];
                        for (d, s) in drow[..cw].iter_mut().zip(srow.iter()) {
                            *d = ((s.red() as u32) << 16) | ((s.green() as u32) << 8) | (s.blue() as u32);
                        }
                        for d in drow[cw..].iter_mut() { *d = 0; }
                    } else {
                        for d in drow.iter_mut() { *d = 0; }
                    }
                }
            } else {
                // bilineare Skalierung (nur noch Ausnahmefall, z.B. Fenster kleiner als 400x300)
                let fx = sw as f32 / pw as f32;
                let fy = sh as f32 / ph as f32;
                for y in 0..ph {
                    let syf = ((y as f32 + 0.5) * fy - 0.5).max(0.0);
                    let sy0 = (syf as u32).min(sh - 1);
                    let sy1 = (sy0 + 1).min(sh - 1);
                    let ty = syf - sy0 as f32;
                    let row = (y * pw) as usize;
                    for x in 0..pw {
                        let sxf = ((x as f32 + 0.5) * fx - 0.5).max(0.0);
                        let sx0 = (sxf as u32).min(sw - 1);
                        let sx1 = (sx0 + 1).min(sw - 1);
                        let tx = sxf - sx0 as f32;
                        let p00 = src[(sy0 * sw + sx0) as usize];
                        let p10 = src[(sy0 * sw + sx1) as usize];
                        let p01 = src[(sy1 * sw + sx0) as usize];
                        let p11 = src[(sy1 * sw + sx1) as usize];
                        let mix = |a: u8, b: u8, c: u8, d: u8| -> u32 {
                            let top = a as f32 + (b as f32 - a as f32) * tx;
                            let bot = c as f32 + (d as f32 - c as f32) * tx;
                            (top + (bot - top) * ty).round() as u32
                        };
                        let r = mix(p00.red(), p10.red(), p01.red(), p11.red());
                        let g = mix(p00.green(), p10.green(), p01.green(), p11.green());
                        let b = mix(p00.blue(), p10.blue(), p01.blue(), p11.blue());
                        buf[row + x as usize] = (r << 16) | (g << 8) | b;
                    }
                }
            }
        });
        let _ = buf.present();
    }

    pub fn warp_mouse(&mut self, x: i32, y: i32) {
        if let Some(w) = &self.app.window {
            let s = w.scale_factor();
            let _ = w.set_cursor_position(PhysicalPosition::new(x as f64 * s, y as f64 * s));
        }
    }
    pub fn set_cursor_visible(&mut self, v: bool) { if let Some(w) = &self.app.window { w.set_cursor_visible(v); } }
    /// Nur für den versteckten Selbsttest: Taste als gedrückt/losgelassen markieren
    pub fn test_set_key(&mut self, k: i32, down: bool) { if down { self.app.keys_down.insert(k); } else { self.app.keys_down.remove(&k); } }
    /// Nur für den versteckten Selbsttest: Mausposition (physisch) setzen
    pub fn test_set_mouse(&mut self, x: i32, y: i32) { let s = self.app.scale(); self.app.mouse_phys = (x as f64 * s, y as f64 * s); }
    pub fn minimize(&mut self) { if let Some(w) = &self.app.window { w.set_minimized(true); } }
}

/// pygame.time.Clock
pub struct Clock {
    last: Option<Instant>,
    frame_times: VecDeque<f64>,
}

impl Default for Clock { fn default() -> Self { Clock::new() } }

impl Clock {
    pub fn new() -> Clock { Clock { last: None, frame_times: VecDeque::new() } }
    /// Wie tick(), aber mit Bruchteilen von Millisekunden (für die feste Spiellogik-Rate)
    pub fn tick_f(&mut self, fps: i32) -> f64 {
        self.tick(fps);
        *self.frame_times.back().unwrap_or(&0.0)
    }
    /// Begrenzt auf `fps` Bilder pro Sekunde, gibt die vergangenen Millisekunden zurück.
    pub fn tick(&mut self, fps: i32) -> i64 {
        let now = Instant::now();
        let mut elapsed = match self.last { Some(l) => now - l, None => Duration::ZERO };
        if fps > 0 {
            let target = Duration::from_secs_f64(1.0 / fps as f64);
            if let Some(l) = self.last {
                if elapsed < target {
                    let wait = target - elapsed;
                    if wait > Duration::from_millis(2) {
                        std::thread::sleep(wait - Duration::from_millis(1));
                    }
                    while l.elapsed() < target { std::hint::spin_loop(); }
                    elapsed = l.elapsed();
                }
            }
        }
        let end = Instant::now();
        self.last = Some(end);
        let ms = elapsed.as_secs_f64() * 1000.0;
        self.frame_times.push_back(ms);
        if self.frame_times.len() > 10 { self.frame_times.pop_front(); }
        ms.round() as i64
    }
    pub fn get_fps(&self) -> f64 {
        let n = self.frame_times.len();
        if n == 0 { return 0.0; }
        let avg: f64 = self.frame_times.iter().sum::<f64>() / n as f64;
        if avg <= 0.0 { 0.0 } else { 1000.0 / avg }
    }
}

impl Default for Display {
    fn default() -> Display { Display::new("JumpNDodge", 800, 600, true, crate::game::icon::window_icon()) }
}
