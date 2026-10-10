use std::ffi::c_void;
use std::fmt;

pub const OUT_W: u32 = 720;
pub const OUT_H: u32 = 480;

#[derive(Debug)]
pub enum GfxError {
    Context(String),
    Shader(String),
    Framebuffer(u32),
}

impl fmt::Display for GfxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GfxError::Context(m) => write!(f, "gl context: {m}"),
            GfxError::Shader(m) => write!(f, "shader: {m}"),
            GfxError::Framebuffer(s) => write!(f, "framebuffer incomplete: 0x{s:x}"),
        }
    }
}

impl std::error::Error for GfxError {}

pub trait Surface {
    fn make_current(&mut self) -> Result<(), GfxError>;
    fn window_size(&self) -> (u32, u32);
    fn swap(&mut self) -> Result<(), GfxError>;
    fn proc_address(&self, name: &str) -> *const c_void;
}

pub fn fit_scale(win_w: u32, win_h: u32) -> u32 {
    (win_w / OUT_W).min(win_h / OUT_H).max(1)
}

pub fn fit_rect(win_w: u32, win_h: u32) -> (i32, i32, i32, i32) {
    let s = fit_scale(win_w, win_h) as i32;
    let (w, h) = (OUT_W as i32 * s, OUT_H as i32 * s);
    ((win_w as i32 - w) / 2, (win_h as i32 - h) / 2, w, h)
}

pub fn blit_rect_fit(panel: (u32, u32), shake: f32) -> (i32, i32, i32, i32) {
    let scale = (panel.0 as f32 / OUT_W as f32).min(panel.1 as f32 / OUT_H as f32);
    let w = (OUT_W as f32 * scale).round() as i32;
    let h = (OUT_H as f32 * scale).round() as i32;
    let dx = (shake * scale).round() as i32;
    (
        (panel.0 as i32 - w) / 2 + dx,
        (panel.1 as i32 - h) / 2,
        w,
        h,
    )
}

pub fn blit_rect(window: (u32, u32), shake: f32) -> (i32, i32, i32, i32) {
    if crate::bezel::framed(window) {
        let (x, y, w, h) = crate::bezel::screen_rect(window);
        let dx = shake * w as f32 / OUT_W as f32;
        return (x + dx.round() as i32, window.1 as i32 - y - h, w, h);
    }
    if window.0 < OUT_W || window.1 < OUT_H {
        return blit_rect_fit(window, shake);
    }
    let (x, y, w, h) = fit_rect(window.0, window.1);
    let dx = shake * fit_scale(window.0, window.1) as f32;
    (x + dx.round() as i32, y, w, h)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OutputScale {
    Fit,
    Integer,
    Stretch,
}

/// Scale the complete 720x480 canvas inside a TV-safe area. The emulator's
/// own aspect ratio remains inside that canvas unless Stretch is requested.
pub fn hdmi_rect(
    window: (u32, u32),
    scale: OutputScale,
    margin: u8,
    shake: f32,
) -> (i32, i32, i32, i32) {
    let percent = u32::from(margin.min(10));
    let inset_x = window.0 * percent / 100;
    let inset_y = window.1 * percent / 100;
    let area = (
        (window.0 - inset_x * 2).max(1),
        (window.1 - inset_y * 2).max(1),
    );
    let (x, y, w, h) = match scale {
        OutputScale::Stretch => (0, 0, area.0 as i32, area.1 as i32),
        OutputScale::Integer if area.0 >= OUT_W && area.1 >= OUT_H => fit_rect(area.0, area.1),
        _ => blit_rect_fit(area, 0.0),
    };
    (
        x + inset_x as i32 + (shake * w as f32 / OUT_W as f32).round() as i32,
        y + inset_y as i32,
        w,
        h,
    )
}
