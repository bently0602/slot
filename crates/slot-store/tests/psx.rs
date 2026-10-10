use slot_store::{scan, Core, Platform};
#[test]
fn playlists_hide_component_discs_but_keep_independent_games() {
    let root = tempfile::tempdir().unwrap();
    let games = root.path().join("Games/PSX");
    std::fs::create_dir_all(&games).unwrap();
    for name in ["Disc 1.cue", "Disc 2.CHD", "Independent.pbp", "Track.bin"] {
        std::fs::write(games.join(name), []).unwrap();
    }
    std::fs::write(
        games.join("Adventure.m3u"),
        "# two discs\r\nDisc 1.cue\r\nDisc 2.CHD\r\n",
    )
    .unwrap();
    let carts = scan(root.path()).unwrap();
    assert_eq!(
        carts.iter().map(|c| c.stem.as_str()).collect::<Vec<_>>(),
        ["Adventure", "Independent"]
    );
    assert!(carts.iter().all(|c| c.platform == Platform::Psx));
    assert_eq!(Core::default_for(Platform::Psx), Core::PcsxRearmed);
    assert!(!Core::Mgba.runs(Platform::Psx));
    assert!(Core::PcsxRearmed.runs(Platform::Psx));
}
