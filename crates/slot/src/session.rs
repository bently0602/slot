use std::path::PathBuf;
use std::time::Duration;

use slot_input::{
    Action, Btn, ControllerMode, ControllerStatus, Gestures, Millis, RawEvent, PLAYERS,
};
use slot_retro::{ButtonMask, Rumble};
use slot_store::Platform;
use slot_ui::FfState;

pub const RUMBLE_MIN_MS: Millis = 100;

use crate::app::{App, Phase, Playback};
use crate::audio::{opened_sink, AudioSink, Ring, Sfx};
use crate::core::open_core;
use crate::emu::{CoreState, EmuHandle, Speed};
use crate::frames::FrameRef;
use crate::input::Pad;
use crate::persist;

pub struct Session {
    root: PathBuf,
    app: App,
    emu: Option<EmuHandle>,
    sink: Box<dyn AudioSink>,
    gestures: Gestures,
    screen_wake_button: Option<Btn>,
    pad: Pad,
    motor: u16,
    pulse: Option<(Millis, u16)>,
    reloading: bool,
    driven: bool,
    docked: bool,
    controller_paused: bool,
    handheld_override: bool,
    multitap: Option<bool>,
    seats: [ButtonMask; PLAYERS],
    music: crate::music::Player,
    music_scan: Option<std::sync::mpsc::Receiver<Vec<slot_store::Album>>>,
    music_restored: bool,
    music_display: Option<bool>,
    music_sleep_playing: bool,
    music_sleeping: bool,
    music_audio: bool,
    music_volume: Option<u8>,
    music_visible: bool,
    music_headphones: bool,
    music_bluetooth: bool,
}

impl Session {
    pub fn boot(root: PathBuf) -> Self {
        Self::boot_with(root, opened_sink())
    }

    pub fn boot_with(root: PathBuf, sink: Box<dyn AudioSink>) -> Self {
        let music = crate::music::Player::spawn(root.clone(), sink.ring());
        let mut app = App::boot(&root);
        app.music.settings = music.snapshot().settings;
        app.music.scanning = true;
        Session {
            music,
            music_scan: Some(Self::scan_music(root.clone())),
            music_restored: false,
            music_display: None,
            music_sleep_playing: false,
            music_sleeping: false,
            music_audio: false,
            music_volume: None,
            music_visible: false,
            music_headphones: app.headphones_active(),
            music_bluetooth: false,
            app,
            root,
            emu: None,
            sink,
            gestures: Gestures::new(),
            screen_wake_button: None,
            pad: Pad::default(),
            motor: 0,
            pulse: None,
            reloading: false,
            driven: false,
            docked: false,
            controller_paused: false,
            handheld_override: false,
            multitap: None,
            seats: Default::default(),
        }
    }

    pub fn play_sfx(&mut self, sfx: Sfx) {
        let ring = self.sink.ring();
        let rate = ring.sample_rate();
        if rate == 0 {
            return;
        }
        let mut samples = sfx.render(rate);
        crate::audio::volume::apply(&mut samples, self.app.output_volume());
        ring.mix(&samples);
    }

    pub fn audio_queued(&self) -> usize {
        self.sink.ring().queued_frames()
    }

    pub fn audio_ring(&self) -> std::sync::Arc<Ring> {
        self.sink.ring()
    }

    pub fn rumble(&mut self, strength: u16) {
        if strength == self.motor {
            return;
        }
        self.motor = strength;
        self.app.set_rumble(strength);
    }

    pub fn core_rumble(&self) -> Option<&Rumble> {
        self.emu.as_ref().map(EmuHandle::rumble)
    }

    pub fn app(&self) -> &App {
        &self.app
    }

    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    pub fn set_docked(&mut self, docked: bool) {
        self.docked = docked;
        self.bridge_link(|app| app.set_docked(docked));
        self.sync_audio_route();
        if self.dozing() {
            self.pad.clear();
            self.app.stop_playback();
            self.sync_pad();
            self.rumble(0);
        }
        if !docked {
            self.controller_paused = false;
            self.handheld_override = false;
        }
        self.sync_speed();
    }

    pub fn controller_paused(&self) -> bool {
        self.controller_paused
    }

    /// Where the external controllers' buttons should go this frame.
    pub fn controller_mode(&self) -> ControllerMode {
        if self.app.choosing_players() {
            ControllerMode::Assign
        } else if self.playing() && !self.app.game_menu_open() {
            ControllerMode::Game
        } else {
            ControllerMode::Menu
        }
    }

    /// The buttons of the players who hold a controller of their own.
    pub fn set_player_buttons(&mut self, held: &[Vec<Btn>; PLAYERS]) {
        self.seats = held.each_ref().map(|buttons| Pad::holding(buttons));
        self.send_pads();
        let Some(emu) = &self.emu else {
            return;
        };
        let tap = self.app.platform().multiplayer() && self.app.multitap_wanted();
        if self.multitap != Some(tap) {
            self.multitap = Some(tap);
            match self.app.platform() {
                Platform::Psx => emu.set_option(
                    "pcsx_rearmed_multitap",
                    if tap { "port 1" } else { "disabled" },
                ),
                // SNES multitaps plug into the second console port. Snes9x reads
                // players 2–5 from consecutive libretro input ports; we expose four.
                Platform::Snes => emu.set_controller_port_device(1, if tap { 257 } else { 1 }),
                _ => {}
            }
        }
    }

    /// Hands the core every player's buttons. The handheld, and any controller seated
    /// with it, plays the handheld's player. A system with one set of controls gives
    /// them to everybody.
    fn send_pads(&self) {
        let Some(emu) = &self.emu else {
            return;
        };
        if self.app.game_menu_open() {
            emu.set_input(ButtonMask(0));
            emu.set_other_inputs([ButtonMask(0); 3]);
            return;
        }
        let mut pads = self.seats;
        if self.app.platform().multiplayer() {
            let own = usize::from(self.app.handheld_player()) - 1;
            pads[own] = ButtonMask(pads[own].0 | self.pad.mask().0);
        } else {
            let all = pads
                .iter()
                .fold(self.pad.mask().0, |held, pad| held | pad.0);
            pads = [ButtonMask(all), ButtonMask(0), ButtonMask(0), ButtonMask(0)];
        }
        emu.set_input(pads[0]);
        emu.set_other_inputs([pads[1], pads[2], pads[3]]);
    }

    pub fn update_controllers(&mut self, status: ControllerStatus) {
        let connected = status.active.is_some();
        self.app.set_controller_status(status);
        if connected {
            self.handheld_override = false;
        }
        let paused = self.docked
            && self.app.controller_config().pause_disconnect
            && !self.app.link_active()
            && !self.handheld_override
            && !connected
            && matches!(
                self.app.phase(),
                Phase::Playing { .. }
                    | Phase::Inserting { .. }
                    | Phase::Polaroids { .. }
                    | Phase::Doze { .. }
            );
        if paused != self.controller_paused {
            self.controller_paused = paused;
            self.gestures = Gestures::new();
            self.pad.clear();
            self.app.stop_playback();
            self.rumble(0);
            self.sync_pad();
            self.sync_speed();
        }
    }

    pub fn emu(&self) -> Option<&EmuHandle> {
        self.emu.as_ref()
    }

    pub fn set_driven(&mut self, driven: bool) {
        self.driven = driven;
        if let Some(emu) = &self.emu {
            emu.set_driven(driven);
        }
    }

    pub fn checkpoint_for_display(&mut self) -> std::io::Result<bool> {
        self.music.sync(crate::music::Command::Checkpoint);
        self.app.music.snapshot = self.music.snapshot();
        if self
            .emu
            .as_ref()
            .is_some_and(|emu| emu.state() == CoreState::Loading)
        {
            return Ok(false);
        }
        if let Some(emu) = &self.emu {
            emu.set_speed(Speed::Paused);
        }
        let saved = self.app.checkpoint_for_display();
        if matches!(saved, Ok(true)) {
            self.emu = None;
            self.rumble(0);
        } else {
            self.sync_speed();
        }
        saved
    }

    pub fn step_emulator(&self, present: Duration, timeout: Duration) -> bool {
        self.emu.as_ref().is_some_and(|emu| {
            emu.tick(present);
            emu.wait_frame(timeout)
        })
    }

    pub fn frame(&self) -> Option<FrameRef> {
        self.emu.as_ref().and_then(|e| e.latest_frame())
    }

    pub fn core_settling(&self) -> bool {
        self.emu
            .as_ref()
            .is_some_and(|e| e.state() == CoreState::Loading)
    }

    pub fn has_core(&self) -> bool {
        self.emu.is_some()
    }

    pub fn frame_ready(&self) -> bool {
        self.emu.as_ref().is_some_and(EmuHandle::frame_ready)
    }

    pub fn frames_published(&self) -> u64 {
        self.emu.as_ref().map_or(0, EmuHandle::published_count)
    }

    pub fn observed_speed(&self) -> Option<Speed> {
        self.emu.as_ref().map(EmuHandle::observed_speed)
    }

    pub fn frames_taken(&self) -> u64 {
        self.emu.as_ref().map_or(0, EmuHandle::frames_taken)
    }

    pub fn game_visible(&self) -> bool {
        self.app.game_visible()
    }

    pub fn feed(&mut self, events: impl IntoIterator<Item = RawEvent>, now: Millis) {
        let mut actions = Vec::new();
        for ev in events {
            // Consume the full wake press, including delayed Menu and Power gestures.
            if let Some(button) = self.screen_wake_button {
                if matches!(ev, RawEvent::Down(b) | RawEvent::Up(b) if b == button) {
                    if ev == RawEvent::Up(button) {
                        self.screen_wake_button = None;
                    }
                    continue;
                }
            }
            if self.app.screen_off() {
                if let RawEvent::Down(button) = ev {
                    if button != Btn::Lid {
                        self.app.restore_screen();
                        self.screen_wake_button = Some(button);
                        self.gestures = Gestures::new();
                        self.pad.clear();
                        actions.clear();
                        continue;
                    }
                }
            }
            if self.controller_paused
                && ev == RawEvent::Down(Btn::Start)
                && matches!(self.app.phase(), Phase::Playing { .. })
                && !self.held()
            {
                self.controller_paused = false;
                self.handheld_override = true;
                self.gestures = Gestures::new();
                self.pad.clear();
                self.sync_speed();
                continue;
            }
            if self
                .app
                .seated_cart()
                .is_some_and(|c| c.platform == Platform::Psx)
            {
                actions.extend(self.gestures.feed_psx(ev, now));
                continue;
            }
            actions.extend(self.gestures.feed(ev, now));
        }
        actions.extend(self.gestures.tick(now));
        for action in actions {
            self.act(action);
        }
        self.sync_pad();
    }

    fn sync_pad(&mut self) {
        for btn in self.app.taken_buttons() {
            self.pad.apply(Action::GbaUp(*btn));
        }
        self.send_pads();
    }

    fn bridge_link(&mut self, f: impl FnOnce(&mut App)) {
        let had_link = self.app.link_active();
        f(&mut self.app);
        if had_link && !self.app.link_active() {
            if let Some(emu) = &self.emu {
                emu.end_link();
            }
        }
    }

    fn act(&mut self, action: Action) {
        if trace() {
            eprintln!("slot: {action:?} in {:?}", self.app.phase());
        }
        let menu = self.overlaid();
        let screen_off = self.app.screen_off();
        self.bridge_link(|app| app.apply(action));
        if !screen_off && self.app.screen_off() {
            self.gestures = Gestures::new();
            self.pad.clear();
        }
        self.sync_music();
        if matches!(
            action,
            Action::VolumeUp | Action::VolumeDown | Action::MuteToggle
        ) {
            if let Some(emu) = &self.emu {
                emu.set_volume(self.app.output_volume());
            }
        }
        if menu || self.overlaid() || self.dozing() {
            self.pad.clear();
        } else if self.app.takes_from_the_game(action) {
            if let Action::GbaDown(btn) | Action::GbaUp(btn) = action {
                self.pad.apply(Action::GbaUp(btn));
            }
        } else {
            self.pad.apply(action);
        }
        self.sync_rumble();
        self.sync_speed();
    }

    pub fn update(&mut self, dt: f32) {
        self.bridge_link(|app| app.update(dt));
        self.sync_audio_route();
        self.sync_music();
        if let Some(emu) = &self.emu {
            emu.set_volume(self.app.output_volume());
        }
        if let Some((client_id, transport)) = self.app.take_link_transport() {
            match &self.emu {
                Some(emu) => match self.app.link_player() {
                    Some(player) => emu.begin_cable(player, transport),
                    None => emu.begin_link(client_id, transport),
                },
                None => eprintln!("slot: link: a transport arrived with no core to run it"),
            }
        }
        if let Some(p) = self.app.take_gb_palette() {
            if let Some(emu) = &self.emu {
                emu.set_option("mgba_gb_colors", p.core_name());
            }
        }
        if let Some(on) = self.app.take_colour_correction() {
            if let Some((key, value)) = crate::core::colour_option(self.app.core(), on) {
                if let Some(emu) = &self.emu {
                    emu.set_option(key, value);
                }
            }
        }
        if let Some((stem, serial)) = self.app.take_link_reload() {
            self.reload_for_link(&stem, serial);
        }
        if self.app.link_active() {
            if self.emu.as_ref().is_some_and(EmuHandle::bios_mismatch) {
                self.bridge_link(|app| app.bios_mismatch());
            } else if self.emu.as_ref().is_some_and(EmuHandle::peer_ended) {
                self.bridge_link(|app| app.peer_ended());
            } else if self.emu.as_ref().is_some_and(EmuHandle::link_lost) {
                self.app.peer_lost();
            }
        }
        if let Some(sfx) = self.app.take_sfx() {
            self.play_sfx(sfx);
        }
        self.sync_core();
        self.sync_reload();
        if let Some(emu) = &self.emu {
            if let Some(index) = self.app.take_disc_change() {
                emu.change_disc(index);
            }
            self.app.set_discs(emu.discs());
        } else {
            self.app.set_discs((0, 0, false));
        }
        self.app
            .set_game_ready(self.emu.as_ref().is_some_and(EmuHandle::has_published));
        self.sync_speed();
        self.sync_rewind_hud();
        self.sync_ff_hud();
        self.sync_rumble();
        self.sync_pad();
    }

    fn sync_rumble(&mut self) {
        let asked = match &self.emu {
            Some(emu) if self.playing() && self.app.rumble_enabled() => emu.rumble().strength(),
            _ => {
                self.pulse = None;
                return self.rumble(0);
            }
        };
        let now = self.app.now();
        let want = match (asked, self.pulse) {
            (0, Some((at, strength))) if now.saturating_sub(at) < RUMBLE_MIN_MS => strength,
            (0, _) => {
                self.pulse = None;
                0
            }
            (asked, Some((at, _))) => {
                self.pulse = Some((at, asked));
                asked
            }
            (asked, None) => {
                self.pulse = Some((now, asked));
                asked
            }
        };
        self.rumble(want);
    }

    fn sync_ff_hud(&mut self) {
        let ff = match self.actually_fast_forwarding() {
            false => FfState::Off,
            true => FfState::Held,
        };
        self.app.set_ff(ff);
    }

    fn sync_rewind_hud(&mut self) {
        let fill = self
            .actually_rewinding()
            .then(|| self.emu.as_ref().map(EmuHandle::rewind_fill))
            .flatten();
        match fill {
            Some(fill) => self.app.show_rewind(fill),
            None => self.app.hide_rewind(),
        }
    }

    fn actually_rewinding(&self) -> bool {
        self.app.playback() == Some(Playback::Rewind) && self.playing() && self.app.may_rewind()
    }

    fn actually_fast_forwarding(&self) -> bool {
        self.app.playback() == Some(Playback::FastForward)
            && self.playing()
            && self.app.may_fast_forward()
    }

    fn inserting(&self) -> bool {
        matches!(self.app.phase(), Phase::Inserting { .. })
    }

    fn showing_polaroids(&self) -> bool {
        matches!(self.app.phase(), Phase::Polaroids { .. })
    }

    fn overlaid(&self) -> bool {
        self.showing_polaroids()
            || self.held()
            || self.app.game_menu_open()
            || self.controller_paused
    }

    fn playing(&self) -> bool {
        matches!(self.app.phase(), Phase::Playing { .. }) && !self.held() && !self.controller_paused
    }

    fn held(&self) -> bool {
        self.app.power_menu().is_some()
            || (self.app.game_menu_open() && self.app.playback().is_none())
            || self.app.shutting_down()
    }

    fn dozing(&self) -> bool {
        matches!(self.app.phase(), Phase::Doze { .. })
    }

    fn ejecting(&self) -> bool {
        matches!(self.app.phase(), Phase::Ejecting { .. })
    }

    fn sync_speed(&self) {
        if let Some(emu) = &self.emu {
            emu.set_fast_steps(u32::from(self.app.ff_speed()));
            emu.set_ff_sound(self.app.ff_sound());
            emu.set_speed(
                if self.inserting()
                    || self.controller_paused
                    || self.ejecting()
                    || self.showing_polaroids()
                    || self.dozing()
                    || (self.held() && !self.app.link_active())
                {
                    Speed::Paused
                } else if self.actually_fast_forwarding() {
                    Speed::Fast
                } else {
                    Speed::Normal
                },
            );
            emu.set_rewinding(self.actually_rewinding());
            emu.set_turbo(self.app.turbo());
            emu.set_run_ahead(self.app.run_ahead());
            emu.set_rewind_recording(self.app.records_rewind());
        }
    }

    fn sync_audio_route(&mut self) {
        self.sink.configure_route(crate::audio::AudioRoute {
            output: self.app.audio_output(),
            docked: self.docked,
            bluetooth: self.app.bluetooth.active,
        });
        self.app
            .set_hdmi_audio(self.docked && self.sink.hdmi_active());
        self.app.set_bluetooth_audio(
            self.app.bluetooth.active.is_some()
                && self.app.audio_output() == slot_store::AudioOutput::Auto
                && self.sink.bluetooth_active(),
        );
    }

    fn no_core() -> bool {
        std::env::var_os("SLOT_NO_CORE").is_some_and(|v| v != "0")
    }

    fn sync_core(&mut self) {
        if Self::no_core() {
            return;
        }
        let stem = match self.app.phase() {
            Phase::Shelf => {
                if self.emu.is_some() {
                    // Trigger releases may arrive after returning to the shelf.
                    // Start the next cart with fresh shortcut/chord state.
                    self.gestures = Gestures::new();
                    self.app.stop_playback();
                    self.pad.clear();
                }
                self.emu = None;
                return;
            }
            Phase::Inserting { cart, .. } => cart.clone(),
            _ => return,
        };
        if self.emu.is_none() {
            let (_, serial) = self.app.link_mode(&stem);
            self.spawn_core(&stem, serial);
        }
        match self.emu.as_ref().map(EmuHandle::state) {
            Some(CoreState::Loading) => {}
            Some(CoreState::Ready) => {
                if let (true, Some(p), Some(emu)) =
                    (self.app.palette_live(), self.app.gb_palette(), &self.emu)
                {
                    emu.set_option("mgba_gb_colors", p.core_name());
                }
                self.app.on_core_ready()
            }
            Some(CoreState::Failed) | None => {
                self.emu = None;
                self.app.on_core_failed();
            }
        }
    }

    fn scan_music(root: PathBuf) -> std::sync::mpsc::Receiver<Vec<slot_store::Album>> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("slot-music-scan".into())
            .spawn(move || {
                let _ = tx.send(slot_store::music::scan(&root));
            })
            .expect("music scanner");
        rx
    }

    pub fn music_display_resume(&mut self, playing: bool) {
        if self.app.prepare_music_display() {
            self.music_display = Some(playing);
        }
    }

    fn prepare_music_audio(&mut self) -> bool {
        if self.music_audio {
            return true;
        }
        self.music.sync(crate::music::Command::Pause);
        match self.sink.open(crate::music::MUSIC_HZ) {
            Ok(()) => {
                self.sink.ring().clear();
                self.music_audio = true;
                self.sync_audio_route();
                true
            }
            Err(e) => {
                self.music.sync(crate::music::Command::Error(e.to_string()));
                false
            }
        }
    }

    fn sync_music(&mut self) {
        use crate::music::Command;
        if let Some(albums) = self.music_scan.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.music_scan = None;
            if self.app.carts().next().is_none() && matches!(self.app.phase(), Phase::Shelf) {
                self.app.shelf_kind = crate::app::ShelfKind::Music;
            }
            self.app.music.replace(albums);
            if !self.music_restored {
                self.music_restored = true;
                let settings = self.music.snapshot().settings;
                let resume = settings.resume.as_ref().and_then(|r| {
                    self.app
                        .music
                        .albums
                        .iter()
                        .find(|a| a.path == r.album)
                        .and_then(|a| {
                            a.tracks
                                .iter()
                                .position(|t| t.path == r.track)
                                .map(|i| (a.clone(), i, r.position))
                        })
                });
                if let Some((album, index, position)) = resume {
                    let display_playing = self.music_display == Some(true);
                    let playing = display_playing && self.app.music_display_can_resume();
                    if !playing || self.prepare_music_audio() {
                        self.music.sync(Command::Play {
                            album,
                            index,
                            position,
                            playing,
                        });
                    }
                    self.app.music.snapshot = self.music.snapshot();
                    if self.music_display.is_some()
                        && matches!(self.app.phase(), Phase::Shelf)
                        && self.app.music.show_playing()
                    {
                        self.app.shelf_kind = crate::app::ShelfKind::Music;
                    }
                } else if settings.resume.is_some() {
                    self.music.sync(Command::Forget);
                }
                self.music_display = None;
            } else {
                let snapshot = self.music.snapshot();
                if snapshot.album.as_ref().is_some_and(|a| {
                    !self.app.music.albums.iter().any(|found| {
                        found.path == a.path
                            && found.tracks.iter().any(|t| {
                                a.tracks
                                    .get(snapshot.index)
                                    .is_some_and(|old| old.path == t.path)
                            })
                    })
                }) {
                    self.music.sync(Command::Forget);
                }
            }
        }
        if self.app.music.rescan && self.music_scan.is_none() {
            self.app.music.rescan = false;
            self.app.music.scanning = true;
            self.music_scan = Some(Self::scan_music(self.root.clone()));
        }
        let forbidden = self.emu.is_some()
            || matches!(
                self.app.phase(),
                Phase::Inserting { .. }
                    | Phase::Playing { .. }
                    | Phase::Ejecting { .. }
                    | Phase::Polaroids { .. }
            )
            || self.app.device_info_requested()
            || self.app.shutting_down();
        let focused = self.app.shelf_kind == crate::app::ShelfKind::Music;
        if !focused {
            self.music_sleep_playing = false;
            if self.music.snapshot().playing {
                self.music.sync(Command::Pause);
            }
        }
        let commands = std::mem::take(&mut self.app.music.commands);
        for command in commands {
            if matches!(&command, Command::Previous | Command::Next)
                && self.music.snapshot().album.is_none()
            {
                continue;
            }
            let starts = matches!(
                &command,
                Command::Play { playing: true, .. }
                    | Command::Toggle
                    | Command::Previous
                    | Command::Next
            );
            if starts && (forbidden || !focused) {
                continue;
            }
            let snapshot = self.music.snapshot();
            if starts && !snapshot.playing && snapshot.error.is_some() {
                self.music_audio = false;
            }
            if starts && !self.prepare_music_audio() {
                continue;
            }
            self.music.sync(command);
        }
        if forbidden && self.music_audio {
            self.music.sync(Command::Stop);
            self.sink.ring().clear();
            if let Err(e) = self.sink.open(crate::audio::GBA_HZ) {
                eprintln!("slot: restoring game audio: {e}");
            }
            self.music_audio = false;
        }
        let headphones = self.app.headphones_active();
        let bluetooth = self.app.bluetooth.active.is_some()
            && self.app.audio_output() == slot_store::AudioOutput::Auto
            && self.sink.bluetooth_active();
        if self.music_audio
            && ((self.music_headphones && !headphones && !self.sink.hdmi_active())
                || (self.music_bluetooth && !bluetooth))
        {
            self.music.sync(Command::Pause);
        }
        self.music_headphones = headphones;
        self.music_bluetooth = bluetooth;
        let volume = self.app.output_volume();
        if self.music_volume != Some(volume) {
            self.music.send(Command::Volume(volume));
            self.music_volume = Some(volume);
        }
        let sleeping = matches!(self.app.phase(), Phase::Doze { .. });
        if sleeping && !self.music_sleeping && self.music_audio {
            self.music_sleep_playing = self.music.snapshot().playing;
            self.music.sync(Command::Pause);
        } else if !sleeping
            && self.music_sleeping
            && self.music_sleep_playing
            && !forbidden
            && focused
        {
            if self.prepare_music_audio() {
                self.music.sync(Command::Toggle);
            }
            self.music_sleep_playing = false;
        }
        self.music_sleeping = sleeping;
        self.app.music.snapshot = self.music.snapshot();
        self.app.sync_music_lid();
        let visible =
            self.app.music_visible() && (self.app.music.flipped || self.app.music.fullscreen);
        if visible != self.music_visible {
            self.music.send(Command::Visible(visible));
            self.music_visible = visible;
        }
    }

    fn spawn_core(&mut self, stem: &str, serial: &'static str) {
        let Some((rom, platform)) = self
            .app
            .seated_cart()
            .filter(|c| c.stem == stem)
            .map(|c| (c.rom.clone(), c.platform))
        else {
            return;
        };
        let core = slot_store::core_for_platform(&self.root, stem, platform);
        self.app.set_core(core);
        self.app.set_platform(platform);
        self.app
            .set_video_mode(crate::video_mode::video_mode_for(&self.root, stem));
        self.app
            .set_run_ahead(crate::run_ahead::run_ahead_for(&self.root, stem));
        self.app.set_link_loaded(serial);
        let resume = (!self.app.starting_clean())
            .then(|| persist::read_resume(&self.root, platform, core, stem))
            .flatten();
        let player = self.app.link_player();
        let palette = crate::core::palette_for(self.app.gb_palette(), platform, &rom, player);
        self.app.set_palette_live(palette.is_some());
        let opened = open_core(
            &self.root,
            core,
            serial,
            self.app.colour_correction(),
            palette,
            player,
        );
        self.app.set_named_core(opened.named);
        let sav = persist::read_sav(&self.root, platform, stem);
        let ring = self.sink.ring();
        let emu = match player.filter(|_| platform == Platform::Gba) {
            Some(p) => EmuHandle::spawn_linked(opened.core, rom, ring, sav, resume, p),
            None => EmuHandle::spawn(opened.core, rom, ring, sav, resume),
        };
        emu.set_volume(self.app.output_volume());
        emu.set_driven(self.driven);
        self.app.set_snapshot(Box::new(emu.snapshot()));
        self.multitap = None;
        self.emu = Some(emu);
    }

    fn reload_for_link(&mut self, stem: &str, serial: &'static str) {
        eprintln!("slot: link: loading {stem} again with gpsp_serial={serial}");
        if self.emu.is_some() {
            self.app.flush_resume();
        }
        self.emu = None;
        self.spawn_core(stem, serial);
        self.reloading = true;
    }

    fn sync_reload(&mut self) {
        if !self.reloading {
            return;
        }
        match self.emu.as_ref().map(EmuHandle::state) {
            Some(CoreState::Loading) => {}
            Some(CoreState::Ready) => {
                self.reloading = false;
                self.app.link_reload_done();
            }
            Some(CoreState::Failed) | None => {
                self.reloading = false;
                self.emu = None;
                self.app.link_reload_failed();
                if let Some((stem, serial)) = self.app.take_link_reload() {
                    self.reload_for_link(&stem, serial);
                }
            }
        }
    }
}

pub(crate) fn trace() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SLOT_TRACE").is_some())
}
