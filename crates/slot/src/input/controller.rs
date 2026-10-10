use slot_input::{Btn, RawEvent};

use super::evdev::{Ev, Hat, EV_ABS, EV_KEY};
use super::mux::InputMux;

/// Linux gamepad codes are positional, unlike the handheld's GPIO key codes.
pub fn button(code: u16) -> Option<Btn> {
    mapped_button(code, false)
}

pub fn mapped_button(code: u16, xinput: bool) -> Option<Btn> {
    Some(match code {
        0x130 => Btn::B, // south: Xbox A / PlayStation Cross
        0x131 => Btn::A, // east: Xbox B / PlayStation Circle
        // xpad uses historical BTN_X/BTN_Y codes rather than NORTH/WEST.
        0x133 => {
            if xinput {
                Btn::Y
            } else {
                Btn::X
            }
        }
        0x134 => {
            if xinput {
                Btn::X
            } else {
                Btn::Y
            }
        }
        0x136 => Btn::L1,
        0x137 => Btn::R1,
        0x138 => Btn::L2,
        0x139 => Btn::R2,
        0x13a => Btn::Select,
        0x13b => Btn::Start,
        0x13c | 0x13d => Btn::Menu, // Guide or left-stick click
        0x220 => Btn::Up,
        0x221 => Btn::Down,
        0x222 => Btn::Left,
        0x223 => Btn::Right,
        0x2c0 => Btn::Left,
        0x2c1 => Btn::Right,
        0x2c2 => Btn::Up,
        0x2c3 => Btn::Down,
        _ => return None,
    })
}

#[derive(Copy, Clone, Debug)]
pub struct AxisRange {
    pub min: i32,
    pub max: i32,
    pub flat: i32,
}

impl AxisRange {
    fn direction(self, value: i32, deadzone: u8) -> i32 {
        let middle = (i64::from(self.min) + i64::from(self.max)) / 2;
        let radius = (i64::from(self.max) - i64::from(self.min)) / 2;
        let dead = (radius * i64::from(deadzone) / 100).max(i64::from(self.flat.max(0)));
        let delta = i64::from(value) - middle;
        if radius <= 0 || delta.abs() <= dead {
            0
        } else {
            delta.signum() as i32
        }
    }

    fn pressed(self, value: i32) -> bool {
        let width = i64::from(self.max) - i64::from(self.min);
        width > 0 && i64::from(value) - i64::from(self.min) > width / 4
    }
}

#[derive(Default)]
pub struct Controller {
    axes: Vec<(u16, AxisRange, Option<Btn>)>,
    held: InputMux,
    hat: Hat,
    xinput: bool,
    deadzone: Option<u8>,
}

impl Controller {
    pub fn set_deadzone(&mut self, percent: u8) {
        self.deadzone = Some(percent.clamp(10, 40));
    }
    pub fn new(xinput: bool) -> Self {
        Self {
            xinput,
            ..Self::default()
        }
    }
    pub fn add_axis(&mut self, code: u16, range: AxisRange, trigger: Option<Btn>) {
        self.axes.push((code, range, trigger));
    }

    pub fn buttons(&self) -> Vec<Btn> {
        self.held.buttons()
    }

    pub fn feed(&mut self, ev: Ev) -> Vec<RawEvent> {
        match ev.kind {
            EV_KEY => {
                let Some(button) = mapped_button(ev.code, self.xinput) else {
                    return Vec::new();
                };
                let event = match ev.value {
                    0 => RawEvent::Up(button),
                    1 => RawEvent::Down(button),
                    _ => return Vec::new(),
                };
                self.held.feed(u64::from(ev.code), event)
            }
            EV_ABS if [0x10, 0x11].contains(&ev.code) => {
                let mut out = Vec::new();
                for edge in self.hat.feed(ev) {
                    out.extend(self.held.feed(0x10000 + u64::from(ev.code), edge));
                }
                out
            }
            EV_ABS => {
                let Some((_, range, trigger)) =
                    self.axes.iter().find(|(code, _, _)| *code == ev.code)
                else {
                    return Vec::new();
                };
                let buttons = match trigger {
                    Some(button) => {
                        if range.pressed(ev.value) {
                            vec![*button]
                        } else {
                            Vec::new()
                        }
                    }
                    None => match (
                        ev.code,
                        range.direction(ev.value, self.deadzone.unwrap_or(25)),
                    ) {
                        (0, -1) => vec![Btn::Left],
                        (0, 1) => vec![Btn::Right],
                        (1, -1) => vec![Btn::Up],
                        (1, 1) => vec![Btn::Down],
                        _ => Vec::new(),
                    },
                };
                self.held.set(0x10000 + u64::from(ev.code), buttons)
            }
            _ => Vec::new(),
        }
    }
}
