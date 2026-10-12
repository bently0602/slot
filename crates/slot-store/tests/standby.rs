use slot_store::{read_slot_state, write_slot_state, SleepShutdown, SlotState};

#[test]
fn standby_choices_round_trip_and_old_or_invalid_keys_preserve_preferences() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Config")).unwrap();
    for choice in SleepShutdown::ALL {
        let state = SlotState {
            sleep_shutdown: choice,
            shutdown_charging: true,
            volume: 37,
            ..SlotState::default()
        };
        write_slot_state(dir.path(), &state).unwrap();
        assert_eq!(read_slot_state(dir.path()), state);
    }
    let path = dir.path().join("Config/slot.state");
    let text = std::fs::read_to_string(&path).unwrap();
    let old = text
        .lines()
        .filter(|s| !s.starts_with("sleep_shutdown=") && !s.starts_with("shutdown_charging="))
        .collect::<Vec<_>>()
        .join("\n");
    for text in [
        old.clone(),
        format!("{old}\nsleep_shutdown=255\nshutdown_charging=bad\n"),
    ] {
        std::fs::write(&path, text).unwrap();
        let state = read_slot_state(dir.path());
        assert_eq!(state.sleep_shutdown, SleepShutdown::Minutes3);
        assert!(!state.shutdown_charging);
        assert_eq!(state.volume, 37);
    }
}
