use slot_store::music::{natural_cmp, scan};
use slot_store::{Artwork, MusicResume, MusicSettings, Repeat};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/music")
        .join(name)
}
fn copy(root: &Path, folder: &str, file: &str, name: &str) {
    let dir = root.join("Games/Music").join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(fixture(file), dir.join(name)).unwrap();
}

#[test]
fn folders_define_albums_and_tags_order_and_name_tracks() {
    let dir = tempfile::tempdir().unwrap();
    copy(dir.path(), "Artist/Album", "mono441.mp3", "01.mp3");
    copy(dir.path(), "Artist/Album", "stereo480.flac", "99.FLAC");
    copy(dir.path(), "Artist/Album/Disc 2", "mono220.mp3", "10.mp3");
    copy(dir.path(), "Artist/Album/Disc 2", "mono220.mp3", "2.mp3");
    std::fs::write(
        dir.path().join("Games/Music/Artist/Album/COVER.PNG"),
        b"invalid art",
    )
    .unwrap();
    copy(dir.path(), ".hidden", "mono441.mp3", "hidden.mp3");
    let albums = scan(dir.path());
    assert_eq!(albums.len(), 2);
    let album = albums.iter().find(|a| a.title == "Test Album").unwrap();
    assert_eq!(album.artist, "Test Artist");
    assert_eq!(album.tracks[0].title, "FLAC Track");
    assert_eq!(album.tracks[1].title, "Tagged Track");
    assert_eq!(album.tracks[1].number, Some(2));
    assert_eq!(album.tracks[1].disc, Some(1));
    assert!((album.tracks[0].duration.unwrap() - 4.0).abs() < 0.01);
    assert!(matches!(&album.artwork,Some(Artwork::Local(p)) if p.ends_with("COVER.PNG")));
    let disc = albums.iter().find(|a| a.path.ends_with("Disc 2")).unwrap();
    assert!(disc.tracks[0].path.ends_with("2.mp3"));
    assert!(disc.tracks[1].path.ends_with("10.mp3"));
}

#[test]
fn duplicate_names_root_tracks_and_corrupt_music_remain_separate() {
    let dir = tempfile::tempdir().unwrap();
    for folder in ["A/Same", "B/Same", ""] {
        copy(dir.path(), folder, "mono220.mp3", "2.mp3");
    }
    std::fs::write(dir.path().join("Games/Music/broken.flac"), b"not a flac").unwrap();
    let albums = scan(dir.path());
    assert_eq!(albums.len(), 3);
    let root = albums
        .iter()
        .find(|a| a.path == Path::new("Games/Music"))
        .unwrap();
    assert_eq!(root.title, "Music");
    assert_eq!(root.tracks.len(), 2);
    assert_eq!(albums.iter().filter(|a| a.title == "Same").count(), 2);
}

#[test]
fn settings_round_trip_and_unsafe_or_corrupt_resumes_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let settings = MusicSettings {
        repeat: Repeat::Album,
        shuffle: true,
        resume: Some(MusicResume {
            album: "Games/Music/Test".into(),
            track: "Games/Music/Test/a.flac".into(),
            position: 12.5,
        }),
        ..Default::default()
    };
    settings.write(dir.path()).unwrap();
    assert_eq!(MusicSettings::read(dir.path()), settings);
    let mut unsafe_settings = settings;
    unsafe_settings.resume.as_mut().unwrap().track = "Games/Music/../../other.mp3".into();
    unsafe_settings.write(dir.path()).unwrap();
    assert!(MusicSettings::read(dir.path()).resume.is_none());
    std::fs::write(dir.path().join("Config/music.json"), b"{broken").unwrap();
    assert_eq!(MusicSettings::read(dir.path()), MusicSettings::default());
}

#[test]
fn natural_sort_handles_padding_and_large_digit_runs() {
    assert!(natural_cmp("track2", "Track10").is_lt());
    assert!(natural_cmp("999999999999999999999999", "1000000000000000000000000").is_lt());
    assert!(natural_cmp("02", "10").is_lt());
}

#[test]
fn embedded_front_cover_is_found_without_local_artwork() {
    let d = tempfile::tempdir().unwrap();
    copy(d.path(), "Embedded", "embedded.mp3", "song.mp3");
    let album = scan(d.path()).remove(0);
    let Some(Artwork::Embedded(track)) = album.artwork else {
        panic!("missing embedded art")
    };
    let bytes = slot_store::music::embedded_art(d.path(), &track).unwrap();
    assert!(bytes.starts_with(b"\x89PNG"));
}
