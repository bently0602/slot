use super::mux::InputMux;
use slot_input::{Btn, ControllerConfig, ControllerInfo, ControllerStatus, RawEvent};
use std::collections::BTreeMap;

struct Pad {
    info: ControllerInfo,
    held: Vec<Btn>,
    blocked: Vec<Btn>,
}

/// Selects one external pad and applies its saved profile. Built-in controls
/// are combined separately, so changing pads cannot release a handheld hold.
#[derive(Default)]
pub struct ControllerHub {
    pads: BTreeMap<u64, Pad>,
    config: ControllerConfig,
    automatic: Option<String>,
    active_source: Option<u64>,
    output: InputMux,
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
        self.pads.insert(
            source,
            Pad {
                info,
                held: Vec::new(),
                blocked: Vec::new(),
            },
        );
        self.sync()
    }

    pub fn disconnect(&mut self, source: u64) -> Vec<RawEvent> {
        if self.active_source == Some(source) {
            self.active_source = None;
        }
        self.pads.remove(&source);
        self.sync()
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
        }
        self.sync()
    }

    pub fn set(&mut self, source: u64, held: Vec<Btn>) -> Vec<RawEvent> {
        if let Some(pad) = self.pads.get_mut(&source) {
            pad.blocked.retain(|b| held.contains(b));
            pad.held = held;
        }
        self.sync()
    }

    fn selected(&self) -> Option<&Pad> {
        let wanted = self.config.preferred.as_ref().or(self.automatic.as_ref())?;
        self.pads
            .get(&self.active_source?)
            .filter(|p| p.info.id == *wanted)
    }

    fn sync(&mut self) -> Vec<RawEvent> {
        let buttons = self
            .selected()
            .map(|pad| {
                let profile = self.config.profile(&pad.info.id);
                pad.held
                    .iter()
                    .filter(|b| !pad.blocked.contains(b))
                    .map(|b| profile.map(*b))
                    .collect()
            })
            .unwrap_or_default();
        self.output.set(0, buttons)
    }

    pub fn status(&self) -> ControllerStatus {
        ControllerStatus {
            connected: self.pads.values().map(|p| p.info.clone()).collect(),
            active: self.selected().map(|p| p.info.id.clone()),
        }
    }

    pub fn deadzone(&self, source: u64) -> u8 {
        self.pads
            .get(&source)
            .map(|p| self.config.profile(&p.info.id).deadzone)
            .unwrap_or(25)
    }
}
