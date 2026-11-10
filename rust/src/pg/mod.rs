//! Kleine pygame-Nachbildung: genau die Teile, die JumpNDodge benutzt.

pub mod display;
pub mod font;
pub mod keys;
pub mod mixer;
pub mod rect;
pub mod surface;

pub use display::{Clock, Display, Event};
pub use font::Font;
pub use keys::*;
pub use mixer::{Channel, Mixer, Sound};
pub use rect::{rect, Num, Rect};
pub use surface::{draw, transform, Color, Pos, RectOpts, Surface};

/// Zwischenablage lesen (pygame.scrap.get(SCRAP_TEXT))
pub fn clipboard_get() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}
