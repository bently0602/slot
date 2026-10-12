use super::mux::InputMux;
use slot_input::{
    Btn, ControllerConfig, ControllerInfo, ControllerMode, ControllerStatus, RawEvent, PLAYERS,
};
use std::collections::BTreeMap;

struct Pad {
    info: ControllerInfo,
    held: Vec<Btn>,
    blocked: Vec<Btn>,
    player: u8,
}

/// Gives every external pad a player and applies its saved profile. Built-in
/// controls are combined separately, so changing pads cannot release a handheld
/// hold.
///
/// Every pad drives the menus. In a game, a pad seated with the handheld shares
/// the handheld's stream, and the other players' buttons are handed out by player
/// so that they reach the game alone. Two pads of one model share an identity, so
/// pads are told apart by their connection.
#[derive(Default)]
pub struct ControllerHub {
    pads: BTreeMap<u64, Pad>,
    config: ControllerConfig,
    automatic: Option<String>,
    active_source: Option<u64>,
    output: InputMux,
    mode: ControllerMode,
    arranged: bool,
    /// The handheld's own player, counted from zero so that it starts as player one.
    handheld: u8,
}

impl ControllerHub {
    pub fn connect(&mut self, source: u64, info: ControllerInfo) -> Vec<RawEvent> {
        if self.automatic.is_none() {
            self.automatic = Some(info.id.clone());
        }
        if self.active_source.is_none()
            && self.config.preferred.as_ref().or(self.automatic.as_ref()) == Some(&info.id)
        {
            self.active_source = Some(source);
        }
        let first = self.active_source == Some(source) || self.arranged;
        let player = self.free_player(if first { 1 } else { 2 });
        self.pads.insert(
            source,
            Pad {
                info,
                held: Vec::new(),
                blocked: Vec::new(),
                player,
            },
        );
        self.sync()
    }

    pub fn disconnect(&mut self, source: u64) -> Vec<RawEvent> {
        if self.active_source == Some(source) {
            self.active_source = None;
        }
        self.pads.remove(&source);
        if self.pads.is_empty() {
            self.arranged = false;
        }
        let mut events = self.output.set(source, Vec::new());
        events.extend(self.sync());
        events
    }

    pub fn configure(&mut self, config: &ControllerConfig) -> Vec<RawEvent> {
        if self.config == *config {
            return Vec::new();
        }
        let selection_changed = self.config.preferred != config.preferred;
        if selection_changed {
            self.automatic = self.pads.values().next().map(|p| p.info.id.clone());
        }
        // Release old mappings and require a fresh press after changing a
        // profile or switching pads. A held face button must not activate UI.
        for pad in self.pads.values_mut() {
            pad.blocked = pad.held.clone();
        }
        self.config = config.clone();
        if selection_changed {
            let wanted = self.config.preferred.as_ref().or(self.automatic.as_ref());
            self.active_source = self
                .pads
                .iter()
                .find(|(_, p)| Some(&p.info.id) == wanted)
                .map(|(source, _)| *source);
            if !self.arranged {
                self.seat_in_order();
            }
        }
        self.sync()
    }

    pub fn set(&mut self, source: u64, held: Vec<Btn>) -> Vec<RawEvent> {
        let choosing = self.mode == ControllerMode::Assign;
        if let Some(pad) = self.pads.get_mut(&source) {
            if choosing {
                let pressed = |b: Btn| held.contains(&b) && !pad.held.contains(&b);
                let player = match (pressed(Btn::Left), pressed(Btn::Right)) {
                    (true, false) => pad.player.saturating_sub(1).max(1),
                    (false, true) => (pad.player + 1).min(PLAYERS as u8),
                    _ => pad.player,
                };
                if player != pad.player {
                    pad.player = player;
                    self.arranged = true;
                }
            }
            pad.blocked.retain(|b| held.contains(b));
            pad.held = held;
        }
        self.sync()
    }

    /// Where the pads' buttons go. Buttons held across the change wait for a fresh
    /// press, so a button a player was holding in the game cannot act on a menu
    /// that opened under it, nor the other way about.
    pub fn set_mode(&mut self, mode: ControllerMode) -> Vec<RawEvent> {
        if self.mode == mode {
            return Vec::new();
        }
        let choosing = self.mode == ControllerMode::Assign || mode == ControllerMode::Assign;
        let handheld = self.handheld + 1;
        for pad in self.pads.values_mut() {
            if choosing || pad.player != handheld {
                pad.blocked = pad.held.clone();
            }
        }
        self.mode = mode;
        self.sync()
    }

    /// A press on the handheld's own buttons. While players are being chosen its left and
    /// right move the handheld between players; those presses are taken, and say so.
    pub fn builtin(&mut self, event: RawEvent) -> bool {
        if self.mode != ControllerMode::Assign {
            return false;
        }
        match event {
            RawEvent::Down(Btn::Left) => self.handheld = self.handheld.saturating_sub(1),
            RawEvent::Down(Btn::Right) => {
                self.handheld = (self.handheld + 1).min(PLAYERS as u8 - 1)
            }
            _ => return false,
        }
        true
    }

    fn free_player(&self, from: u8) -> u8 {
        (from..=PLAYERS as u8)
            .find(|n| self.pads.values().all(|p| p.player != *n))
            .unwrap_or(PLAYERS as u8)
    }

    fn seat_in_order(&mut self) {
        let active = self.active_source;
        let mut next = 2;
        for (source, pad) in &mut self.pads {
            if Some(*source) == active {
                pad.player = 1;
            } else {
                pad.player = next.min(PLAYERS as u8);
                next += 1;
            }
        }
    }

    fn selected(&self) -> Option<&Pad> {
        let wanted = self.config.preferred.as_ref().or(self.automatic.as_ref())?;
        self.pads
            .get(&self.active_source?)
            .filter(|p| p.info.id == *wanted)
    }

    fn mapped(&self, pad: &Pad) -> Vec<Btn> {
        let profile = self.config.profile(&pad.info.id);
        pad.held
            .iter()
            .filter(|b| !pad.blocked.contains(b))
            .map(|b| profile.map(*b))
            .collect()
    }

    fn sync(&mut self) -> Vec<RawEvent> {
        let handheld = self.handheld + 1;
        let streams: Vec<(u64, Vec<Btn>)> = self
            .pads
            .iter()
            .map(|(source, pad)| {
                let mut buttons = self.mapped(pad);
                match self.mode {
                    ControllerMode::Menu => {}
                    ControllerMode::Assign => {
                        buttons.retain(|b| !matches!(b, Btn::Left | Btn::Right))
                    }
                    ControllerMode::Game if pad.player == handheld => {}
                    ControllerMode::Game => buttons.retain(|b| *b == Btn::Menu),
                }
                (*source, buttons)
            })
            .collect();
        streams
            .into_iter()
            .flat_map(|(source, buttons)| self.output.set(source, buttons))
            .collect()
    }

    /// Each player's buttons while a game has the pads, but for the handheld's player:
    /// pads seated with the handheld are in its stream.
    pub fn players(&self) -> [Vec<Btn>; PLAYERS] {
        let mut players: [Vec<Btn>; PLAYERS] = Default::default();
        if self.mode != ControllerMode::Game {
            return players;
        }
        let apart = self.pads.values().filter(|p| p.player != self.handheld + 1);
        for pad in apart {
            let held = &mut players[usize::from(pad.player) - 1];
            for button in self.mapped(pad) {
                if button != Btn::Menu && !held.contains(&button) {
                    held.push(button);
                }
            }
        }
        players
    }

    pub fn status(&self) -> ControllerStatus {
        ControllerStatus {
            connected: self.pads.values().map(|p| p.info.clone()).collect(),
            active: self.selected().map(|p| p.info.id.clone()),
            players: self.pads.values().map(|p| p.player).collect(),
            handheld: self.handheld + 1,
        }
    }

    pub fn deadzone(&self, source: u64) -> u8 {
        self.pads
            .get(&source)
            .map(|p| self.config.profile(&p.info.id).deadzone)
            .unwrap_or(25)
    }
}
