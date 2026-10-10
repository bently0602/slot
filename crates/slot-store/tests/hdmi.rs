use slot_store::{read_slot_state, write_slot_state, HdmiScale, SlotState};

#[test]
fn hdmi_settings_round_trip_and_old_or_invalid_keys_keep_other_preferences() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Config")).unwrap();
    let state = SlotState {
        hdmi_auto: false,
        hdmi_1080p: true,
        hdmi_scale: HdmiScale::Integer,
        hdmi_margin: 10,
        docked_lid: false,
        audio_output: slot_store::AudioOutput::Hdmi,
        volume: 37,
        coverflow: true,
        ..SlotState::default()
    };
    write_slot_state(dir.path(), &state).unwrap();
    assert_eq!(read_slot_state(dir.path()), state);
    let path = dir.path().join("Config/slot.state");
    let text = std::fs::read_to_string(&path).unwrap();
    let old = text
        .lines()
        .filter(|s| {
            !s.starts_with("hdmi_")
                && !s.starts_with("docked_lid=")
                && !s.starts_with("audio_output=")
        })
        .collect::<Vec<_>>()
        .join("\n");
    for text in [
        old.clone(),
        format!("{old}\nhdmi_auto=garbage\nhdmi_1080p=8\nhdmi_scale=bad\nhdmi_margin=255\ndocked_lid=3\naudio_output=oops\n"),
    ] {
        std::fs::write(&path, text).unwrap();
        let read = read_slot_state(dir.path());
        assert!(read.hdmi_auto);
        assert!(!read.hdmi_1080p);
        assert_eq!(read.hdmi_scale, HdmiScale::Fit);
        assert_eq!(read.hdmi_margin, 0);
        assert!(read.docked_lid);
        assert_eq!(read.audio_output, slot_store::AudioOutput::Auto);
        assert_eq!(read.volume, 37);
        assert!(read.coverflow);
    }
}
