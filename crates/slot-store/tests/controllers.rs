use slot_input::{ControllerConfig, ControllerProfile};
use slot_store::{read_controller_config, write_controller_config};

#[test]
fn controller_profiles_round_trip_with_unicode_and_separators_in_device_names() {
    let root = tempfile::tempdir().unwrap();
    let config = ControllerConfig {
        preferred: Some("usb|2dc8|Ultimate,β=\n".into()),
        pause_disconnect: false,
        profiles: vec![ControllerProfile {
            id: "usb|2dc8|Ultimate,β=\n".into(),
            name: "8BitDo Ultimate,β\n".into(),
            swap_ab: true,
            swap_xy: true,
            deadzone: 40,
        }],
    };
    write_controller_config(root.path(), &config).unwrap();
    assert_eq!(read_controller_config(root.path()), config);
}

#[test]
fn missing_or_partly_invalid_config_keeps_safe_defaults_and_valid_profiles() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        read_controller_config(root.path()),
        ControllerConfig::default()
    );
    std::fs::create_dir(root.path().join("Config")).unwrap();
    std::fs::write(root.path().join("Config/controllers.conf"), "pause_disconnect=nonsense\npreferred=odd\nprofile=61,41,1,0,30\nprofile=62,42,1,0,99\nprofile=63,43,2,0,25\nprofile=ββ,44,1,0,25\nfuture_setting=1\n").unwrap();
    let config = read_controller_config(root.path());
    assert!(config.pause_disconnect);
    assert_eq!(config.preferred, None);
    assert_eq!(config.profiles.len(), 1);
    assert_eq!(config.profile("a").deadzone, 30);
}
