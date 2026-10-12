use gilrs::{Axis, EventType, Gilrs};
use slot_input::{
    Btn, ControllerConfig, ControllerInfo, ControllerMode, ControllerStatus, InputSource, Millis,
    RawEvent, PLAYERS,
};
use std::collections::BTreeMap;
use winit::event::WindowEvent;
use winit::keyboard::{KeyCode, PhysicalKey};

use super::controller_hub::ControllerHub;
use super::keys::key_to_btn;
use super::mux::InputMux;
use super::pad::pad_to_btn;

pub struct HostInput {
    pending: Vec<RawEvent>,
    pad: Option<Gilrs>,
    lid_closed: bool,
    held_keys: Vec<Btn>,
    mux: InputMux,
    pads: BTreeMap<usize, InputMux>,
    hub: ControllerHub,
}

impl HostInput {
    pub fn new() -> Self {
        let pad = Gilrs::new().ok();
        let mut hub = ControllerHub::default();
        if let Some(pads) = &pad {
            for (id, gamepad) in pads.gamepads() {
                hub.connect(
                    usize::from(id) as u64 + 1,
                    ControllerInfo {
                        id: format!("{:02x?}|{}", gamepad.uuid(), gamepad.name()),
                        name: gamepad.name().into(),
                    },
                );
            }
        }
        HostInput {
            pending: Vec::new(),
            pad,
            lid_closed: false,
            held_keys: Vec::new(),
            mux: InputMux::default(),
            pads: BTreeMap::new(),
            hub,
        }
    }

    pub fn on_window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Focused(false) => self.release_keys(),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                self.key(code, event.state.is_pressed(), event.repeat);
            }
            _ => {}
        }
    }

    fn release_keys(&mut self) {
        for btn in std::mem::take(&mut self.held_keys) {
            self.pending.extend(self.mux.feed(0, RawEvent::Up(btn)));
        }
    }

    pub fn key(&mut self, code: KeyCode, pressed: bool, repeat: bool) {
        if repeat {
            return;
        }
        let Some(btn) = key_to_btn(code) else {
            return;
        };
        if btn == Btn::Lid {
            if pressed {
                self.lid_closed = !self.lid_closed;
                self.pending.push(if self.lid_closed {
                    RawEvent::Down(Btn::Lid)
                } else {
                    RawEvent::Up(Btn::Lid)
                });
            }
            return;
        }
        match pressed {
            true if !self.held_keys.contains(&btn) => self.held_keys.push(btn),
            true => {}
            false => self.held_keys.retain(|b| *b != btn),
        }
        self.push(btn, pressed);
    }

    fn push(&mut self, btn: Btn, pressed: bool) {
        if pressed && self.hub.builtin(RawEvent::Down(btn)) {
            return;
        }
        if pressed {
            crate::latency::read();
        }
        self.pending.extend(self.mux.feed(
            0,
            if pressed {
                RawEvent::Down(btn)
            } else {
                RawEvent::Up(btn)
            },
        ));
    }

    fn drain_pad(&mut self) {
        let Some(pad) = self.pad.as_mut() else {
            return;
        };
        while let Some(ev) = pad.next_event() {
            let id = usize::from(ev.id);
            let source = id as u64 + 1;
            let held = self.pads.entry(id).or_default();
            match ev.event {
                EventType::Connected => {
                    let gamepad = pad.gamepad(ev.id);
                    let changes = self.hub.connect(
                        source,
                        ControllerInfo {
                            id: format!("{:02x?}|{}", gamepad.uuid(), gamepad.name()),
                            name: gamepad.name().into(),
                        },
                    );
                    for event in changes {
                        self.pending.extend(self.mux.feed(u64::MAX, event));
                    }
                }
                EventType::ButtonPressed(b, _) | EventType::ButtonReleased(b, _) => {
                    if let Some(btn) = pad_to_btn(b) {
                        let down = matches!(ev.event, EventType::ButtonPressed(..));
                        let event = if down {
                            crate::latency::read();
                            RawEvent::Down(btn)
                        } else {
                            RawEvent::Up(btn)
                        };
                        held.feed(b as u64, event);
                        for event in self.hub.set(source, held.buttons()) {
                            self.pending.extend(self.mux.feed(u64::MAX, event));
                        }
                    }
                }
                EventType::AxisChanged(axis @ (Axis::LeftStickX | Axis::LeftStickY), value, _) => {
                    let (negative, positive) = if axis == Axis::LeftStickX {
                        (Btn::Left, Btn::Right)
                    } else {
                        (Btn::Down, Btn::Up)
                    };
                    let deadzone = f32::from(self.hub.deadzone(source)) / 100.0;
                    let buttons = if value < -deadzone {
                        vec![negative]
                    } else if value > deadzone {
                        vec![positive]
                    } else {
                        Vec::new()
                    };
                    held.set(0x10000 + axis as u64, buttons);
                    for event in self.hub.set(source, held.buttons()) {
                        self.pending.extend(self.mux.feed(u64::MAX, event));
                    }
                }
                EventType::Disconnected => {
                    self.pads.remove(&id);
                    for event in self.hub.disconnect(source) {
                        self.pending.extend(self.mux.feed(u64::MAX, event));
                    }
                }
                _ => {}
            }
        }
    }
}

impl Default for HostInput {
    fn default() -> Self {
        HostInput::new()
    }
}

impl InputSource for HostInput {
    fn configure_controllers(&mut self, config: &ControllerConfig) {
        for event in self.hub.configure(config) {
            self.pending.extend(self.mux.feed(u64::MAX, event));
        }
    }
    fn controller_status(&self) -> ControllerStatus {
        self.hub.status()
    }
    fn set_controller_mode(&mut self, mode: ControllerMode) {
        for event in self.hub.set_mode(mode) {
            self.pending.extend(self.mux.feed(u64::MAX, event));
        }
    }
    fn player_buttons(&self) -> [Vec<Btn>; PLAYERS] {
        self.hub.players()
    }
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.drain_pad();
        std::mem::take(&mut self.pending)
    }
}
