/// Blend two RGB colors by a given factor `t` (0.0 = `from`, 1.0 = `to`).
pub fn blend(from: (u8, u8, u8), to: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| -> u8 { (a as f32 + (b as f32 - a as f32) * t).round() as u8 };
    (lerp(from.0, to.0), lerp(from.1, to.1), lerp(from.2, to.2))
}
