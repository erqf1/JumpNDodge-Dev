//! pygame.Rect in Rust: gleiche Felder und Methoden, ganzzahlig wie in pygame.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Kurzform wie `pygame.Rect(x, y, w, h)` - nimmt beliebige Zahlen und schneidet ab wie pygame.
pub fn rect<A: Num, B: Num, C: Num, D: Num>(x: A, y: B, w: C, h: D) -> Rect {
    Rect { x: x.to_i(), y: y.to_i(), w: w.to_i(), h: h.to_i() }
}

/// Zahlen, die pygame an Rect/Positionen akzeptiert (int und float, float wird abgeschnitten).
pub trait Num: Copy {
    fn to_i(self) -> i32;
    fn to_f(self) -> f64;
}
impl Num for i32 { fn to_i(self) -> i32 { self } fn to_f(self) -> f64 { self as f64 } }
impl Num for i64 { fn to_i(self) -> i32 { self as i32 } fn to_f(self) -> f64 { self as f64 } }
impl Num for u32 { fn to_i(self) -> i32 { self as i32 } fn to_f(self) -> f64 { self as f64 } }
impl Num for usize { fn to_i(self) -> i32 { self as i32 } fn to_f(self) -> f64 { self as f64 } }
impl Num for f32 { fn to_i(self) -> i32 { self as i32 } fn to_f(self) -> f64 { self as f64 } }
impl Num for f64 { fn to_i(self) -> i32 { self as i32 } fn to_f(self) -> f64 { self } }

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect { Rect { x, y, w, h } }
    pub fn left(&self) -> i32 { self.x }
    pub fn top(&self) -> i32 { self.y }
    pub fn right(&self) -> i32 { self.x + self.w }
    pub fn bottom(&self) -> i32 { self.y + self.h }
    pub fn width(&self) -> i32 { self.w }
    pub fn height(&self) -> i32 { self.h }
    pub fn centerx(&self) -> i32 { self.x + self.w / 2 }
    pub fn centery(&self) -> i32 { self.y + self.h / 2 }
    pub fn center(&self) -> (i32, i32) { (self.centerx(), self.centery()) }
    pub fn topleft(&self) -> (i32, i32) { (self.x, self.y) }
    pub fn topright(&self) -> (i32, i32) { (self.right(), self.y) }
    pub fn bottomleft(&self) -> (i32, i32) { (self.x, self.bottom()) }
    pub fn bottomright(&self) -> (i32, i32) { (self.right(), self.bottom()) }
    pub fn midtop(&self) -> (i32, i32) { (self.centerx(), self.y) }
    pub fn midbottom(&self) -> (i32, i32) { (self.centerx(), self.bottom()) }
    pub fn midleft(&self) -> (i32, i32) { (self.x, self.centery()) }
    pub fn midright(&self) -> (i32, i32) { (self.right(), self.centery()) }
    pub fn size(&self) -> (i32, i32) { (self.w, self.h) }

    pub fn set_left(&mut self, v: i32) { self.x = v; }
    pub fn set_top(&mut self, v: i32) { self.y = v; }
    pub fn set_right(&mut self, v: i32) { self.x = v - self.w; }
    pub fn set_bottom(&mut self, v: i32) { self.y = v - self.h; }
    pub fn set_centerx(&mut self, v: i32) { self.x = v - self.w / 2; }
    pub fn set_centery(&mut self, v: i32) { self.y = v - self.h / 2; }
    pub fn set_center(&mut self, c: (i32, i32)) { self.set_centerx(c.0); self.set_centery(c.1); }
    pub fn set_topleft(&mut self, c: (i32, i32)) { self.x = c.0; self.y = c.1; }
    pub fn set_midbottom(&mut self, c: (i32, i32)) { self.set_centerx(c.0); self.set_bottom(c.1); }
    pub fn set_midtop(&mut self, c: (i32, i32)) { self.set_centerx(c.0); self.y = c.1; }
    pub fn set_topright(&mut self, c: (i32, i32)) { self.set_right(c.0); self.y = c.1; }
    pub fn set_bottomleft(&mut self, c: (i32, i32)) { self.x = c.0; self.set_bottom(c.1); }
    pub fn set_bottomright(&mut self, c: (i32, i32)) { self.set_right(c.0); self.set_bottom(c.1); }
    pub fn set_midleft(&mut self, c: (i32, i32)) { self.x = c.0; self.set_centery(c.1); }
    pub fn set_midright(&mut self, c: (i32, i32)) { self.set_right(c.0); self.set_centery(c.1); }
    pub fn set_width(&mut self, v: i32) { self.w = v; }
    pub fn set_height(&mut self, v: i32) { self.h = v; }

    /// Wie pygame: rechte/untere Kante gehört nicht mehr dazu.
    pub fn collidepoint<P: Into<(i32, i32)>>(&self, p: P) -> bool {
        let (px, py) = p.into();
        let (x0, x1) = if self.w >= 0 { (self.x, self.x + self.w) } else { (self.x + self.w, self.x) };
        let (y0, y1) = if self.h >= 0 { (self.y, self.y + self.h) } else { (self.y + self.h, self.y) };
        px >= x0 && px < x1 && py >= y0 && py < y1
    }
    pub fn colliderect(&self, o: &Rect) -> bool {
        if self.w == 0 || self.h == 0 || o.w == 0 || o.h == 0 {
            return false;
        }
        let a = self.normalized();
        let b = o.normalized();
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    }
    pub fn normalized(&self) -> Rect {
        let mut r = *self;
        if r.w < 0 { r.x += r.w; r.w = -r.w; }
        if r.h < 0 { r.y += r.h; r.h = -r.h; }
        r
    }
    pub fn inflate(&self, dx: i32, dy: i32) -> Rect {
        Rect { x: self.x - dx / 2, y: self.y - dy / 2, w: self.w + dx, h: self.h + dy }
    }
    pub fn inflate_ip(&mut self, dx: i32, dy: i32) { *self = self.inflate(dx, dy); }
    pub fn moved(&self, dx: i32, dy: i32) -> Rect { Rect { x: self.x + dx, y: self.y + dy, ..*self } }
    pub fn move_ip(&mut self, dx: i32, dy: i32) { self.x += dx; self.y += dy; }
    pub fn copy(&self) -> Rect { *self }
    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        let r = self.right().max(o.right());
        let b = self.bottom().max(o.bottom());
        Rect { x, y, w: r - x, h: b - y }
    }
    pub fn clip(&self, o: &Rect) -> Rect {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        if r <= x || b <= y {
            return Rect { x: self.x, y: self.y, w: 0, h: 0 };
        }
        Rect { x, y, w: r - x, h: b - y }
    }
    /// pygame Rect.clamp: in `o` hineinschieben (zentriert, falls zu groß).
    pub fn clamp(&self, o: &Rect) -> Rect {
        let mut r = *self;
        if r.w >= o.w { r.x = o.x + o.w / 2 - r.w / 2; } else if r.x < o.x { r.x = o.x; } else if r.right() > o.right() { r.x = o.right() - r.w; }
        if r.h >= o.h { r.y = o.y + o.h / 2 - r.h / 2; } else if r.y < o.y { r.y = o.y; } else if r.bottom() > o.bottom() { r.y = o.bottom() - r.h; }
        r
    }
    pub fn clamp_ip(&mut self, o: &Rect) { *self = self.clamp(o); }
    pub fn contains(&self, o: &Rect) -> bool {
        o.x >= self.x && o.y >= self.y && o.right() <= self.right() && o.bottom() <= self.bottom()
    }
    pub fn fit(&self, o: &Rect) -> Rect {
        let xr = self.w as f64 / o.w as f64;
        let yr = self.h as f64 / o.h as f64;
        let m = xr.max(yr);
        let w = (self.w as f64 / m) as i32;
        let h = (self.h as f64 / m) as i32;
        let x = o.x + (o.w - w) / 2;
        let y = o.y + (o.h - h) / 2;
        Rect { x, y, w, h }
    }
}

impl From<(i32, i32, i32, i32)> for Rect {
    fn from(t: (i32, i32, i32, i32)) -> Rect { Rect::new(t.0, t.1, t.2, t.3) }
}
