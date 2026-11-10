//! Fenstersymbol: das Python-Spiel lädt *.ico neben der exe, sonst ein gezeichnetes "JD".

pub fn window_icon() -> Option<(Vec<u8>, u32, u32)> {
    static ICO: &[u8] = include_bytes!("../../../icon.ico");
    decode_ico_largest(ICO)
}

/// Größtes PNG- oder 32-bit-BMP-Bild aus einer .ico-Datei als RGBA
fn decode_ico_largest(data: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if data.len() < 6 { return None; }
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;
    let mut best: Option<(u32, usize, usize)> = None;
    for i in 0..count {
        let e = 6 + i * 16;
        if e + 16 > data.len() { break; }
        let w = if data[e] == 0 { 256 } else { data[e] as u32 };
        let size = u32::from_le_bytes([data[e + 8], data[e + 9], data[e + 10], data[e + 11]]) as usize;
        let off = u32::from_le_bytes([data[e + 12], data[e + 13], data[e + 14], data[e + 15]]) as usize;
        if w <= 64 && best.map(|b| w > b.0).unwrap_or(true) { best = Some((w, off, size)); }
    }
    let (_, off, size) = best?;
    let img = data.get(off..off + size)?;
    if img.starts_with(b"\x89PNG") {
        let pm = tiny_skia::Pixmap::decode_png(img).ok()?;
        let (w, h) = (pm.width(), pm.height());
        let rgba: Vec<u8> = pm.pixels().iter().flat_map(|p| { let c = p.demultiply(); [c.red(), c.green(), c.blue(), c.alpha()] }).collect();
        return Some((rgba, w, h));
    }
    // BMP (BITMAPINFOHEADER, 32 bit, unten nach oben)
    let w = i32::from_le_bytes(img[4..8].try_into().ok()?) as u32;
    let h2 = i32::from_le_bytes(img[8..12].try_into().ok()?) as u32;
    let bpp = u16::from_le_bytes([img[14], img[15]]);
    if bpp != 32 { return None; }
    let h = h2 / 2;
    let hdr = u32::from_le_bytes(img[0..4].try_into().ok()?) as usize;
    let px = &img[hdr..];
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let si = (((h - 1 - y) * w + x) * 4) as usize;
            let di = ((y * w + x) * 4) as usize;
            if si + 3 >= px.len() { return None; }
            out[di] = px[si + 2];
            out[di + 1] = px[si + 1];
            out[di + 2] = px[si];
            out[di + 3] = px[si + 3];
        }
    }
    Some((out, w, h))
}
