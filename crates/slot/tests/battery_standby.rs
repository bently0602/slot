mod common;

use common::{app_playing_with, app_playing_with_charge, boot, panel, tmp_root_with_carts};
use slot::app::{App, Phase};
use slot::persist::{self, Snapshot};
use slot_input::{Action, Btn};
use slot_power::{Battery, Charge};
use slot_store::{
    read_slot_state, write_slot_state, Core, Platform, SleepShutdown, SlotState, StateRing,
};
use slot_ui::{QuickRow, QuickValue, Toast};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

fn battery(percent: u8, charge: Charge) -> Battery {
    Battery { percent, charge }
}

struct Counting {
    count: Arc<AtomicUsize>,
    serialize: bool,
    trusted: bool,
}

impl Snapshot for Counting {
    fn state(&self) -> Option<Vec<u8>> {
        let n = self.count.fetch_add(1, Ordering::Relaxed);
        self.serialize.then(|| vec![n as u8; 32])
    }
    fn save_ram(&self) -> Option<Vec<u8>> {
        Some(vec![7; 32])
    }
    fn thumb(&self) -> Option<Vec<u8>> {
        None
    }
    fn load(&self, _: Vec<u8>) {}
    fn resume_trusted(&self) -> bool {
        self.trusted
    }
}

fn counted(root: &std::path::Path, serialize: bool, trusted: bool) -> (App, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let app = app_playing_with(
        root,
        "Emerald",
        Box::new(Counting {
            count: count.clone(),
            serialize,
            trusted,
        }),
    );
    (app, count)
}

fn ring(root: &std::path::Path) -> StateRing {
    StateRing::new(root, Platform::Gba, Core::Mgba, "Emerald")
}

#[test]
fn low_battery_saves_durably_once_and_rearms_after_charging_or_recovery() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, count) = counted(d.path(), true, true);
    a.on_battery(battery(11, Charge::Discharging));
    assert_eq!(count.load(Ordering::Relaxed), 0);
    a.on_battery(battery(10, Charge::Discharging));
    assert_eq!(ring(d.path()).read_resume().unwrap(), Some(vec![0; 32]));
    assert_eq!(
        persist::read_sav(d.path(), Platform::Gba, "Emerald"),
        Some(vec![7; 32])
    );
    assert!(ring(d.path()).list().unwrap().is_empty());
    assert_eq!(a.toast(), Some(Toast::BatterySaved));
    assert!(!a.powering_off());
    for percent in [9, 11, 15, 10] {
        a.on_battery(battery(percent, Charge::Discharging));
    }
    assert_eq!(count.load(Ordering::Relaxed), 1);
    a.on_battery(battery(9, Charge::Charging));
    a.on_battery(battery(9, Charge::Discharging));
    assert_eq!(count.load(Ordering::Relaxed), 2);
    a.on_battery(battery(16, Charge::Discharging));
    a.on_battery(battery(10, Charge::Discharging));
    assert_eq!(count.load(Ordering::Relaxed), 3);
}

#[test]
fn resume_write_failure_reports_failure_and_retries_after_thirty_seconds() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let states = d.path().join("States/GBA/mgba/Emerald/resume.state");
    std::fs::create_dir_all(&states).unwrap();
    let (mut a, count) = counted(d.path(), true, true);
    a.set_power(common::panel_with_battery(d.path(), Duration::from_secs(180), 1, 10).0);
    a.on_battery(battery(10, Charge::Discharging));
    assert_eq!(a.toast(), Some(Toast::BatterySaveFailed));
    let retry_at = a.now() + 30_000;
    assert_eq!(
        persist::read_sav(d.path(), Platform::Gba, "Emerald"),
        Some(vec![7; 32])
    );
    a.tick_ms(retry_at - 1);
    assert_eq!(count.load(Ordering::Relaxed), 1);
    std::fs::remove_dir(states).unwrap();
    a.tick_ms(retry_at);
    assert_eq!(count.load(Ordering::Relaxed), 2);
    assert_eq!(a.toast(), Some(Toast::BatterySaved));
    assert!(ring(d.path()).read_resume().unwrap().is_some());
}

#[test]
fn missing_serialization_preserves_battery_ram_without_claiming_a_resume() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, _) = counted(d.path(), false, true);
    a.on_battery(battery(10, Charge::Unknown));
    assert_eq!(a.toast(), Some(Toast::BatterySaveFailed));
    assert_eq!(
        persist::read_sav(d.path(), Platform::Gba, "Emerald"),
        Some(vec![7; 32])
    );
    assert!(ring(d.path()).read_resume().unwrap().is_none());
}

#[test]
fn untrusted_resume_does_not_overwrite_existing_progress() {
    let d = tmp_root_with_carts(&["Emerald"]);
    ring(d.path()).write_resume(&[5; 32]).unwrap();
    let (mut a, count) = counted(d.path(), true, false);
    a.on_battery(battery(10, Charge::Discharging));
    assert_eq!(a.toast(), Some(Toast::BatterySaveFailed));
    assert_eq!(count.load(Ordering::Relaxed), 0);
    assert_eq!(ring(d.path()).read_resume().unwrap(), Some(vec![5; 32]));
}

#[test]
fn charging_and_full_never_trigger_low_battery_actions() {
    for charge in [Charge::Charging, Charge::Full] {
        let d = tmp_root_with_carts(&["Emerald"]);
        let (mut a, count) = counted(d.path(), true, true);
        a.on_battery(battery(1, charge));
        assert_eq!(count.load(Ordering::Relaxed), 0);
        assert!(!a.powering_off());
    }
}

#[test]
fn linked_play_defers_the_early_save_until_the_link_ends() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, count) = counted(d.path(), true, true);
    a.begin_link(0);
    a.on_battery(battery(10, Charge::Discharging));
    assert_eq!(count.load(Ordering::Relaxed), 0);
    a.end_link();
    a.on_battery(battery(10, Charge::Discharging));
    assert_eq!(count.load(Ordering::Relaxed), 1);
}

#[test]
fn every_new_game_gets_a_low_battery_checkpoint() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut a, count) = counted(d.path(), true, true);
    a.on_battery(battery(10, Charge::Discharging));
    a.apply(Action::Eject);
    a.update(2.0);
    a.update(2.0);
    assert!(matches!(a.phase(), Phase::Shelf));
    a.apply(Action::Insert);
    a.on_core_ready();
    a.update(2.0);
    let before = count.load(Ordering::Relaxed);
    a.on_battery(battery(10, Charge::Discharging));
    assert_eq!(count.load(Ordering::Relaxed), before + 1);
    assert_eq!(a.toast(), Some(Toast::BatterySaved));
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

fn select(a: &mut App, row: QuickRow) {
    for _ in 0..QuickRow::ALL.len() {
        if a.quick_menu() == Some(row) {
            return;
        }
        a.apply(Action::GbaDown(Btn::Down));
    }
    panic!("row {row:?} not found");
}

#[test]
fn battery_menu_changes_wrap_persist_and_apply_to_the_next_sleep() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.set_power(panel(d.path(), Duration::from_secs(180)).0);
    a.apply(Action::QuickMenu);
    select(&mut a, QuickRow::Battery);
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(a.quick_menu(), Some(QuickRow::BatteryStatus));
    assert_eq!(
        a.quick_value(QuickRow::SleepShutdown),
        Some(QuickValue::Minutes3)
    );
    select(&mut a, QuickRow::SleepShutdown);
    a.apply(Action::GbaDown(Btn::Left));
    assert_eq!(
        read_slot_state(d.path()).sleep_shutdown,
        SleepShutdown::Minute1
    );
    a.apply(Action::GbaDown(Btn::B));
    assert_eq!(a.quick_menu(), Some(QuickRow::Battery));
    a.apply(Action::GbaDown(Btn::B));
    a.apply(Action::PowerTap);
    a.tick_ms(59_999);
    assert!(!a.powering_off());
    a.tick_ms(60_000);
    assert!(a.powering_off());
}

#[test]
fn never_disables_standby_shutdown_but_not_critical_battery_protection() {
    let d = tmp_root_with_carts(&["Emerald"]);
    common::clocked(d.path());
    let state = SlotState {
        sleep_shutdown: SleepShutdown::Never,
        clock_set: true,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &state).unwrap();
    let mut a = App::boot(d.path());
    a.set_power(panel(d.path(), state.sleep_shutdown.duration()).0);
    a.apply(Action::PowerTap);
    a.tick_ms(3_600_000);
    assert!(!a.powering_off());
    a.on_battery(battery(5, Charge::Discharging));
    assert!(a.powering_off());
}

#[test]
fn charging_at_the_deadline_keeps_standby_alive_and_unplug_starts_a_fresh_delay() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, _) = app_playing_with_charge(d.path(), "Emerald");
    // This test platform's stopped clock opens the clock picker; the power tap
    // still enters standby and uses its one-minute timeout.
    a.apply(Action::PowerTap);
    a.tick_ms(59_000);
    charge.store(2, Ordering::Relaxed);
    a.tick_ms(60_000);
    assert!(!a.powering_off());
    a.tick_ms(120_000);
    assert!(!a.powering_off());
    charge.store(1, Ordering::Relaxed);
    a.tick_ms(121_000);
    a.tick_ms(179_999);
    assert!(!a.powering_off());
    a.tick_ms(180_000);
    assert!(a.powering_off());
}

#[test]
fn battery_status_distinguishes_charge_states_and_missing_telemetry() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = boot(d.path());
    assert_eq!(a.battery_text(), "Unavailable");
    for (charge, label) in [
        (Charge::Charging, "Charging"),
        (Charge::Full, "Full"),
        (Charge::Discharging, "Discharging"),
        (Charge::Unknown, "Status Unknown"),
    ] {
        a.on_battery(battery(68, charge));
        assert_eq!(a.battery_text(), format!("68% / {label}"));
    }
}
