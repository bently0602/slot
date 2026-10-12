use crate::draw::{OUT_H, OUT_W};
use crate::plate::UndoFace;
use crate::power_menu::MENU_INK;
use crate::text;

/// The handheld and the most controllers the screen lists. A fifth controller and
/// later play but are not shown.
pub const PLAYER_ROWS: usize = 5;
pub const PLAYER_COLUMNS: usize = 4;

const W: u32 = OUT_W;
const H: u32 = OUT_H;
const COLUMN_W: u32 = 132;
const COLUMN_GAP: u32 = 12;
const COLUMNS_W: u32 = COLUMN_W * PLAYER_COLUMNS as u32 + COLUMN_GAP * (PLAYER_COLUMNS as u32 - 1);
const LEFT: u32 = (W - COLUMNS_W) / 2;
const TITLE_H: u32 = 52;
const HEADER_H: u32 = 48;
const ROW_H: u32 = 62;
const LEGEND_H: u32 = 44;
const PAD_W: u32 = 52;
const PAD_H: u32 = 28;
const NAME_PX: f32 = 15.0;
const NAME_CHARS: usize = 16;
const DIM_INK: [u8; 3] = [0x9a, 0x9a, 0xa4];
const COLUMN_FILL: [u8; 4] = [0xff, 0xff, 0xff, 0x14];
const SEAT_FILL: [u8; 4] = [0xff, 0xff, 0xff, 0x30];

/// The handheld or a connected controller, and the player it has chosen, from 1.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Seat<'a> {
    pub name: &'a str,
    pub player: u8,
}

/// The left edge and width of a player's column on the screen.
pub fn player_column(player: u8) -> (u32, u32) {
    let at = u32::from(player.clamp(1, PLAYER_COLUMNS as u8)) - 1;
    (LEFT + at * (COLUMN_W + COLUMN_GAP), COLUMN_W)
}

/// The top edge and height of a controller's row on the screen.
pub fn player_row(row: usize) -> (u32, u32) {
    let rows = PLAYER_ROWS as u32;
    let block = TITLE_H + HEADER_H + ROW_H * rows + LEGEND_H;
    let top = (H - block) / 2 + TITLE_H + HEADER_H;
    (top + ROW_H * row.min(PLAYER_ROWS - 1) as u32, ROW_H)
}

/// The whole player-select board: a column for each player and, in a row of its own, each
/// controller's icon standing in the column it has chosen.
pub fn players_face(seats: &[Seat]) -> UndoFace {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    let (first_row, _) = player_row(0);
    let top = first_row - HEADER_H;
    let column_h = HEADER_H + ROW_H * PLAYER_ROWS as u32;
    words(
        &mut rgba,
        "Players",
        0,
        top - TITLE_H,
        W,
        TITLE_H,
        26.0,
        MENU_INK,
    );
    for player in 1..=PLAYER_COLUMNS as u8 {
        let (x, w) = player_column(player);
        fill(&mut rgba, x, top, w, column_h, COLUMN_FILL);
        let ink = if seats.iter().any(|s| s.player == player) {
            MENU_INK
        } else {
            DIM_INK
        };
        words(
            &mut rgba,
            &player.to_string(),
            x,
            top,
            w,
            HEADER_H,
            30.0,
            ink,
        );
    }
    let pad = pad_cover();
    for (row, seat) in seats.iter().take(PLAYER_ROWS).enumerate() {
        let (x, w) = player_column(seat.player);
        let (y, h) = player_row(row);
        fill(&mut rgba, x + 6, y + 4, w - 12, h - 8, SEAT_FILL);
        stamp(
            &mut rgba,
            &pad,
            x + (w - PAD_W) / 2,
            y + 8,
            PAD_W,
            PAD_H,
            MENU_INK,
        );
        let name: String = seat.name.chars().take(NAME_CHARS).collect();
        words(
            &mut rgba,
            &name,
            x + 6,
            y + h - 26,
            w - 12,
            22,
            NAME_PX,
            MENU_INK,
        );
    }
    let legend = "Left / Right on your own controller picks your player.  B: Back";
    words(
        &mut rgba,
        legend,
        0,
        top + column_h,
        W,
        LEGEND_H,
        NAME_PX,
        DIM_INK,
    );
    UndoFace { rgba, w: W, h: H }
}

fn fill(dst: &mut [u8], x: u32, y: u32, w: u32, h: u32, colour: [u8; 4]) {
    for yy in y..(y + h).min(H) {
        for xx in x..(x + w).min(W) {
            over(dst, xx, yy, [colour[0], colour[1], colour[2]], colour[3]);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn words(dst: &mut [u8], label: &str, x: u32, y: u32, w: u32, h: u32, px: f32, ink: [u8; 3]) {
    let Some(font) = text::label_font() else {
        return;
    };
    let layout = text::fit(font, label, w as f32, 1, px, px * 0.7);
    stamp(dst, &text::coverage(w, h, &layout), x, y, w, h, ink);
}

/// A gamepad, drawn here because the symbol font carries only the HUD's glyphs.
fn pad_cover() -> Vec<u8> {
    const SUB: u32 = 4;
    let inside = |x: f32, y: f32| {
        let disc = |cx: f32, cy: f32, r: f32| (x - cx).powi(2) + (y - cy).powi(2) <= r * r;
        let bar = |x0: f32, y0: f32, x1: f32, y1: f32| x >= x0 && x < x1 && y >= y0 && y < y1;
        let body = bar(9.0, 2.0, 43.0, 22.0)
            || disc(9.0, 11.0, 9.0)
            || disc(43.0, 11.0, 9.0)
            || disc(9.0, 20.0, 8.0)
            || disc(43.0, 20.0, 8.0);
        let grips = bar(18.0, 22.0, 34.0, 30.0) && !disc(26.0, 32.0, 11.0);
        let cross = bar(8.0, 9.5, 20.0, 13.5) || bar(12.0, 5.5, 16.0, 17.5);
        let buttons = disc(35.0, 13.5, 2.6) || disc(41.0, 8.5, 2.6);
        (body && !grips) && !cross && !buttons
    };
    let mut cover = Vec::with_capacity((PAD_W * PAD_H) as usize);
    for y in 0..PAD_H {
        for x in 0..PAD_W {
            let mut hits = 0;
            for sy in 0..SUB {
                for sx in 0..SUB {
                    let at = |p: u32, s: u32| p as f32 + (s as f32 + 0.5) / SUB as f32;
                    hits += u32::from(inside(at(x, sx), at(y, sy)));
                }
            }
            cover.push((hits * 255 / (SUB * SUB)) as u8);
        }
    }
    cover
}

fn stamp(dst: &mut [u8], cov: &[u8], x: u32, y: u32, w: u32, h: u32, ink: [u8; 3]) {
    for yy in 0..h {
        for xx in 0..w {
            let a = cov[(yy * w + xx) as usize];
            if a > 0 && x + xx < W && y + yy < H {
                over(dst, x + xx, y + yy, ink, a);
            }
        }
    }
}

fn over(dst: &mut [u8], x: u32, y: u32, ink: [u8; 3], alpha: u8) {
    let at = ((y * W + x) * 4) as usize;
    let (a, under) = (u32::from(alpha), u32::from(dst[at + 3]));
    let out = a + under * (255 - a) / 255;
    if out == 0 {
        return;
    }
    for c in 0..3 {
        let below = u32::from(dst[at + c]) * under * (255 - a) / 255;
        dst[at + c] = ((u32::from(ink[c]) * a + below) / out) as u8;
    }
    dst[at + 3] = out as u8;
}
