use std::collections::BTreeMap;

use slot_input::{Btn, RawEvent};

/// A release from one controller must not release another controller's button.
#[derive(Default)]
pub struct InputMux {
    held: BTreeMap<u64, Vec<Btn>>,
}

impl InputMux {
    pub fn feed(&mut self, source: u64, event: RawEvent) -> Vec<RawEvent> {
        let mut buttons = self.held.get(&source).cloned().unwrap_or_default();
        match event {
            RawEvent::Down(button) if !buttons.contains(&button) => buttons.push(button),
            RawEvent::Up(button) => buttons.retain(|b| *b != button),
            _ => return Vec::new(),
        }
        self.set(source, buttons)
    }

    pub fn set(&mut self, source: u64, buttons: Vec<Btn>) -> Vec<RawEvent> {
        let before = self.buttons();
        if buttons.is_empty() {
            self.held.remove(&source);
        } else {
            self.held.insert(source, buttons);
        }
        let after = self.buttons();
        before
            .iter()
            .filter(|b| !after.contains(b))
            .copied()
            .map(RawEvent::Up)
            .chain(
                after
                    .iter()
                    .filter(|b| !before.contains(b))
                    .copied()
                    .map(RawEvent::Down),
            )
            .collect()
    }

    pub fn buttons(&self) -> Vec<Btn> {
        let mut all = Vec::new();
        for button in self.held.values().flatten() {
            if !all.contains(button) {
                all.push(*button);
            }
        }
        all
    }
}
