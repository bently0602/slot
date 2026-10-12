mod common;
use slot::app::{App, Phase, ShelfKind};
use slot::audio::{AudioSink, Ring, StubSink};
use slot::music::{Analysis, Analyzer, Command, Player, Queue, MUSIC_HZ};
use slot::music_ui::{draw_visualizer, panel_face, Browser, PANEL_H, PANEL_W};
use slot::session::Session;
use slot_gfx::{Draw, TexId};
use slot_input::{Action, Btn, RawEvent};
use slot_store::{Album, MusicSettings, Repeat, VisualizerStyle};
use std::path::Path;
use std::sync::{atomic::Ordering, Arc};
use std::time::{Duration, Instant};

fn library(files: &[&str]) -> tempfile::TempDir {
    let d = common::tmp_root_with_carts(&[]);
    common::clocked(d.path());
    let album = d.path().join("Games/Music/Artist/Album");
    std::fs::create_dir_all(&album).unwrap();
    for file in files {
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/music")
                .join(file),
            album.join(file),
        )
        .unwrap();
    }
    d
}
fn wait(mut condition: impl FnMut() -> bool) {
    let start = Instant::now();
    while !condition() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "music condition timed out"
        );
        std::thread::sleep(Duration::from_millis(3));
    }
}
fn player(d: &tempfile::TempDir) -> (Player, Arc<Ring>, Album) {
    let ring = Arc::new(Ring::new(8192));
    ring.reopen(MUSIC_HZ);
    ring.clear();
    let album = slot_store::music::scan(d.path()).remove(0);
    (Player::spawn(d.path().into(), ring.clone()), ring, album)
}
fn drain(ring: &Ring) {
    let n = ring.queued_frames();
    ring.fill(&mut vec![0; n * 2]);
}
fn session(d: &tempfile::TempDir) -> (Session, StubSink) {
    let mut sink = StubSink::new();
    sink.open(32768).unwrap();
    let copy = sink.clone();
    let mut s = Session::boot_with(d.path().into(), Box::new(sink));
    wait(|| {
        s.update(0.016);
        !s.app().music.scanning
    });
    s.app_mut().shelf_kind = ShelfKind::Music;
    (s, copy)
}
fn press(s: &mut Session, button: Btn, now: u64) {
    s.feed([RawEvent::Down(button), RawEvent::Up(button)], now);
    s.update(0.016);
}

#[test]
fn actual_mp3_flac_decode_resample_and_transport_preserve_position() {
    for file in [
        "mono441.mp3",
        "mono220.mp3",
        "stereo480.flac",
        "stereo960.flac",
        "mono320.flac",
    ] {
        let d = library(&[file]);
        let (p, ring, a) = player(&d);
        p.sync(Command::Play {
            album: a,
            index: 0,
            position: 0.0,
            playing: true,
        });
        wait(|| ring.queued_frames() > 500);
        let mut out = vec![0; ring.queued_frames() * 2];
        ring.fill(&mut out);
        assert!(out.iter().any(|s| *s != 0), "silent decode: {file}");
        p.sync(Command::Pause);
        let paused = p.snapshot();
        assert!(!paused.playing);
        assert!(paused.position > 0.0);
        assert_eq!(ring.queued_frames(), 0);
        p.sync(Command::Seek(0.3));
        let sought = p.snapshot().position;
        assert!((sought - paused.position - 0.3).abs() < 0.001);
        p.sync(Command::Toggle);
        wait(|| ring.queued_frames() > 0);
        p.sync(Command::Pause);
        assert!((p.snapshot().position - sought).abs() < 0.02);
        assert!(
            p.snapshot().error.is_none(),
            "{file}: {:?}",
            p.snapshot().error
        );
        assert!(
            (MusicSettings::read(d.path()).resume.unwrap().position - p.snapshot().position).abs()
                < 0.01
        );
    }
}

#[test]
fn queue_shuffle_repeat_and_previous_history() {
    let mut q = Queue::new(8, 3, true);
    let mut current = 3;
    let mut visited = vec![current];
    while let Some(next) = q.next(8, current, Repeat::Off, true, true) {
        visited.push(next);
        current = next;
    }
    assert_eq!(visited.len(), 8);
    let mut sorted = visited.clone();
    sorted.sort();
    assert_eq!(sorted, (0..8).collect::<Vec<_>>());
    assert_eq!(q.previous(current), visited[6]);
    let mut q = Queue::new(3, 1, false);
    assert_eq!(q.next(3, 1, Repeat::Track, false, false), Some(1));
    assert_eq!(q.next(3, 1, Repeat::Track, false, true), Some(2));
    assert_eq!(q.next(3, 2, Repeat::Album, false, false), Some(0));
}

#[test]
fn corrupt_album_stops_after_one_pass_even_with_repeat_track() {
    let d = library(&["mono441.mp3"]);
    std::fs::write(
        d.path().join("Games/Music/Artist/Album/mono441.mp3"),
        b"bad mp3",
    )
    .unwrap();
    let (p, _, album) = player(&d);
    p.sync(Command::Settings(MusicSettings {
        repeat: Repeat::Track,
        ..Default::default()
    }));
    p.sync(Command::Play {
        album,
        index: 0,
        position: 0.0,
        playing: true,
    });
    wait(|| !p.snapshot().playing);
    assert_eq!(p.snapshot().error_id, 1);
}

#[test]
fn audio_backpressure_retains_samples_and_stop_is_prompt() {
    let d = library(&["stereo480.flac"]);
    let (p, ring, album) = player(&d);
    p.sync(Command::Play {
        album,
        index: 0,
        position: 0.0,
        playing: true,
    });
    wait(|| ring.queued_frames() == ring.capacity_frames());
    std::thread::sleep(Duration::from_millis(30));
    assert!(p.snapshot().position < 0.001);
    assert_eq!(ring.overruns(), 0);
    let start = Instant::now();
    p.sync(Command::Stop);
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(ring.queued_frames(), 0);
}

#[test]
fn analyzer_finds_tone_and_silence_without_nonfinite_values() {
    let mut a = Analyzer::new();
    let tone: Vec<f32> = (0..4096)
        .flat_map(|i| {
            let v = (std::f32::consts::TAU * 1000.0 * i as f32 / 48000.0).sin() * 0.5;
            [v, v]
        })
        .collect();
    let state = a.feed(&tone, 48000).unwrap();
    let peak = state
        .bars
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0;
    assert!((15..=19).contains(&peak), "tone band {peak}");
    assert!(state.waveform.iter().any(|s| s.abs() > 0.3));
    for _ in 0..50 {
        a.feed(&vec![0.0; 4096], 48000);
    }
    let silence = a.feed(&vec![0.0; 4096], 48000).unwrap();
    assert!(silence.bars.iter().all(|b| b.is_finite() && *b < 0.01));
}

#[test]
fn music_controls_do_not_insert_carts_and_game_start_remains_core_picker() {
    let d = library(&["mono441.mp3"]);
    let mut a = App::new(Vec::new());
    a.music.replace(slot_store::music::scan(d.path()));
    a.shelf_kind = ShelfKind::Music;
    a.apply(Action::GbaDown(Btn::A));
    assert!(a.music.flipped);
    assert!(matches!(a.phase(), Phase::Shelf));
    for action in [
        Action::SaveState,
        Action::LoadState,
        Action::FfStart,
        Action::RewindStart,
        Action::Polaroids,
    ] {
        a.apply(action);
        assert!(matches!(a.phase(), Phase::Shelf));
    }
    a.apply(Action::GbaDown(Btn::Start));
    assert_eq!(a.music.commands.len(), 1);
    assert!(a.core_picker().is_none());
}

#[test]
fn session_browsing_sleep_and_lid_policy_preserve_music() {
    let d = library(&["stereo480.flac"]);
    let (mut s, sink) = session(&d);
    press(&mut s, Btn::Start, 100);
    wait(|| {
        s.update(0.016);
        sink.ring().queued_frames() > 100
    });
    assert!(s.app().music.snapshot.playing);
    press(&mut s, Btn::Menu, 200);
    std::thread::sleep(Duration::from_millis(300));
    s.feed([], 600);
    s.update(0.016);
    assert!(s.app().music.snapshot.playing);
    s.feed([RawEvent::Down(Btn::Lid)], 700);
    s.update(0.016);
    assert!(!matches!(s.app().phase(), Phase::Doze { .. }));
    assert!(s.app().music.snapshot.playing);
    s.app_mut().music.settings.keep_playing_closed = false;
    s.app_mut().sync_music_lid();
    s.update(0.016);
    assert!(matches!(s.app().phase(), Phase::Doze { .. }));
    assert!(!s.app().music.snapshot.playing);
    s.feed([RawEvent::Up(Btn::Lid)], 800);
    s.update(0.016);
    assert!(s.app().music.snapshot.playing);
    drain(&sink.ring());
}

#[test]
fn startup_restores_paused_and_display_resume_restores_playing() {
    let d = library(&["stereo480.flac"]);
    {
        let (p, _, album) = player(&d);
        p.sync(Command::Play {
            album,
            index: 0,
            position: 1.2,
            playing: false,
        });
    }
    let (s, _) = session(&d);
    assert!(!s.app().music.snapshot.playing);
    assert!((s.app().music.snapshot.position - 1.2).abs() < 0.01);
    drop(s);
    let mut sink = StubSink::new();
    sink.open(32768).unwrap();
    let mut s = Session::boot_with(d.path().into(), Box::new(sink));
    s.music_display_resume(true);
    wait(|| {
        s.update(0.016);
        !s.app().music.scanning
    });
    assert!(s.app().music.snapshot.playing);
}

#[test]
fn visualizer_geometry_and_flip_are_bounded_and_distinct() {
    let d = library(&["mono441.mp3"]);
    let mut b = Browser::default();
    let mut albums = slot_store::music::scan(d.path());
    let duplicate = albums[0].clone();
    albums.push(duplicate);
    b.replace(albums);
    b.covers.insert(0, TexId::from_raw(1));
    b.covers.insert(1, TexId::from_raw(2));
    let mut draws = Vec::new();
    b.draw(0, &mut draws);
    assert_eq!(
        draws
            .iter()
            .filter(|d| matches!(d,Draw::Yawed {yaw,..} if *yaw==0.0))
            .count(),
        1
    );
    b.flipped = true;
    for i in 0..30 {
        b.update(1.0 / 60.0, i * 16);
    }
    assert_eq!(b.flip, 1.0);
    b.panel = Some(TexId::from_raw(3));
    b.flip = 0.5;
    let mut midpoint = Vec::new();
    b.draw(0, &mut midpoint);
    assert!(midpoint.iter().any(
        |d| matches!(d, Draw::Yawed {yaw, x, y, w, h, tex: Some(tex), ..}
        if *tex == TexId::from_raw(3) && (yaw.abs() - std::f32::consts::FRAC_PI_2).abs() < 0.001
        && [*x,*y,*w,*h].iter().all(|v| v.is_finite()))
    ));
    b.flip = 1.0;
    assert_eq!(panel_face(&b).len(), (PANEL_W * PANEL_H * 4) as usize);
    let analysis = Analysis {
        bars: [0.5; 32],
        peaks: [0.6; 32],
        ..Default::default()
    };
    let mut bars = Vec::new();
    draw_visualizer(
        &analysis,
        VisualizerStyle::Bars,
        40.0,
        96.0,
        640.0,
        300.0,
        &mut bars,
    );
    let mut wave = Vec::new();
    draw_visualizer(
        &analysis,
        VisualizerStyle::Waveform,
        40.0,
        96.0,
        640.0,
        300.0,
        &mut wave,
    );
    assert_ne!(bars.len(), wave.len());
    for draw in bars.iter().chain(&wave) {
        if let Draw::Rect { x, y, w, h, .. } = draw {
            assert!(*x >= 40.0 && *y >= 96.0 && x + w <= 681.0 && y + h <= 397.0);
        }
    }
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/music-panel.png");
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    image::save_buffer(
        &output,
        &panel_face(&b),
        PANEL_W,
        PANEL_H,
        image::ColorType::Rgba8,
    )
    .unwrap();
}

#[test]
fn full_screen_controls_do_not_change_hidden_track_selection() {
    let d = library(&["mono441.mp3", "stereo480.flac"]);
    let mut browser = Browser::default();
    browser.replace(slot_store::music::scan(d.path()));
    browser.flipped = true;
    browser.fullscreen = true;
    browser.input(Action::GbaDown(Btn::Down), 0);
    browser.input(Action::GbaDown(Btn::A), 1);
    assert_eq!(browser.track, 0);
    assert!(browser.commands.is_empty());
    browser.input(Action::GbaDown(Btn::Right), 2);
    assert_eq!(browser.settings.style, VisualizerStyle::Waveform);
}

#[test]
fn lid_closed_end_and_power_button_use_normal_sleep() {
    let d = library(&["mono441.mp3"]);
    let mut app = App::new(Vec::new());
    let (power, light) = common::panel(d.path(), Duration::from_secs(60));
    app.set_power(power);
    app.music.snapshot.playing = true;
    app.shelf_kind = ShelfKind::Music;
    app.apply(Action::LidClose);
    assert_eq!(light.load(Ordering::Relaxed), 0);
    assert!(matches!(app.phase(), Phase::Shelf));
    app.apply(Action::PowerTap);
    assert!(matches!(app.phase(), Phase::Doze { .. }));
    app.apply(Action::PowerTap);
    app.music.snapshot.playing = false;
    app.sync_music_lid();
    assert!(matches!(app.phase(), Phase::Doze { .. }));
}

#[test]
fn seek_clamps_without_changing_tracks_and_previous_restarts() {
    let d = library(&["mono441.mp3", "stereo480.flac"]);
    let (p, ring, album) = player(&d);
    p.sync(Command::Play {
        album,
        index: 0,
        position: 3.5,
        playing: false,
    });
    p.sync(Command::Previous);
    assert_eq!(p.snapshot().index, 0);
    assert_eq!(p.snapshot().position, 0.0);
    p.sync(Command::Pause);
    p.sync(Command::Seek(-100.0));
    assert_eq!(p.snapshot().position, 0.0);
    p.sync(Command::Seek(100.0));
    let end = p.snapshot();
    assert_eq!(end.position, end.duration.unwrap());
    assert_eq!(end.index, 0);
    assert!(!end.playing);
    assert_eq!(ring.queued_frames(), 0);
}

#[test]
fn rescan_preserves_browsing_and_playback_by_file_path() {
    let d = library(&["mono441.mp3", "stereo480.flac"]);
    let (mut s, sink) = session(&d);
    press(&mut s, Btn::Start, 100);
    wait(|| sink.ring().queued_frames() > 100);
    s.update(0.016);
    let before = s.app().music.snapshot.resume().unwrap();
    s.app_mut().music.track = 1;
    let selected = s.app().music.selected().unwrap().tracks[1].path.clone();
    std::fs::write(d.path().join("Games/Music/Artist/Album/99-bad.mp3"), b"bad").unwrap();
    s.app_mut().music.rescan = true;
    s.update(0.016);
    wait(|| {
        s.update(0.016);
        !s.app().music.scanning
    });
    assert!(s.app().music.snapshot.playing);
    assert_eq!(s.app().music.snapshot.resume().unwrap().track, before.track);
    assert_eq!(
        s.app().music.selected().unwrap().tracks[s.app().music.track].path,
        selected
    );
    assert_eq!(s.app().music.selected().unwrap().tracks.len(), 3);
}

#[test]
fn headphone_removal_pauses_and_reconnection_does_not_resume() {
    let d = library(&["stereo480.flac"]);
    let (mut s, sink) = session(&d);
    let (mut app, jack) = common::app_playing_with_jack(d.path(), "Fake");
    app.apply(Action::Eject);
    for _ in 0..180 {
        app.update(1.0 / 60.0);
    }
    assert!(matches!(app.phase(), Phase::Shelf));
    app.music = std::mem::take(&mut s.app_mut().music);
    app.shelf_kind = ShelfKind::Music;
    *s.app_mut() = app;
    jack.store(true, Ordering::Relaxed);
    s.app_mut().tick_ms(6_000);
    s.update(0.016);
    press(&mut s, Btn::Start, 5_000);
    wait(|| {
        s.update(0.016);
        sink.ring().queued_frames() > 100
    });
    assert!(s.app().music.snapshot.playing);
    jack.store(false, Ordering::Relaxed);
    s.app_mut().tick_ms(7_000);
    s.update(0.016);
    assert!(!s.app().music.snapshot.playing);
    assert_eq!(sink.ring().queued_frames(), 0);
    jack.store(true, Ordering::Relaxed);
    s.app_mut().tick_ms(8_000);
    s.update(0.016);
    assert!(!s.app().music.snapshot.playing);
}

#[test]
fn bluetooth_loss_pauses_music_even_when_hdmi_or_a_wired_jack_is_available() {
    struct RoutedSink {
        sink: StubSink,
        route: slot::audio::AudioRoute,
    }
    impl AudioSink for RoutedSink {
        fn open(&mut self, rate: u32) -> Result<(), slot::audio::AudioError> {
            self.sink.open(rate)
        }
        fn ring(&self) -> Arc<Ring> {
            self.sink.ring()
        }
        fn configure_route(&mut self, route: slot::audio::AudioRoute) {
            self.route = route;
        }
        fn bluetooth_active(&self) -> bool {
            self.route.wants_bluetooth()
        }
        fn hdmi_active(&self) -> bool {
            self.route.docked && !self.route.wants_bluetooth()
        }
    }
    let d = library(&["stereo480.flac"]);
    let mut sink = StubSink::new();
    sink.open(32768).unwrap();
    let ring = sink.ring();
    let mut s = Session::boot_with(
        d.path().into(),
        Box::new(RoutedSink {
            sink,
            route: Default::default(),
        }),
    );
    wait(|| {
        s.update(0.016);
        !s.app().music.scanning
    });
    s.app_mut().shelf_kind = ShelfKind::Music;
    s.set_docked(true);
    s.app_mut().bluetooth.active = Some([1; 6]);
    s.update(0.016);
    assert!(s.app().headphones_active());
    press(&mut s, Btn::Start, 100);
    wait(|| {
        s.update(0.016);
        ring.queued_frames() > 100
    });
    assert!(s.app().music.snapshot.playing);
    s.app_mut().bluetooth.active = None;
    s.update(0.016);
    assert!(!s.app().music.snapshot.playing);
    assert_eq!(ring.queued_frames(), 0);
    s.app_mut().bluetooth.active = Some([1; 6]);
    s.update(0.016);
    assert!(!s.app().music.snapshot.playing);
}

#[test]
fn game_launch_stops_music_before_restoring_game_audio() {
    let d = library(&["stereo480.flac"]);
    let (mut s, sink) = session(&d);
    press(&mut s, Btn::Start, 100);
    wait(|| sink.ring().queued_frames() > 100);
    std::fs::write(d.path().join("Games/GBA/Fake.gba"), [0u8; 256]).unwrap();
    std::fs::write(d.path().join("Games/GBA/Other.gba"), [0u8; 256]).unwrap();
    let mut app = App::boot(d.path());
    app.music = std::mem::take(&mut s.app_mut().music);
    *s.app_mut() = app;
    s.app_mut().apply(Action::Insert);
    s.update(0.016);
    assert!(!s.app().music.snapshot.playing);
    assert_eq!(sink.ring().sample_rate(), slot::audio::GBA_HZ);
    assert_eq!(
        MusicSettings::read(d.path())
            .resume
            .unwrap()
            .track
            .extension()
            .unwrap(),
        "flac"
    );
}

#[test]
fn output_that_stops_draining_pauses_with_an_error() {
    let d = library(&["stereo480.flac"]);
    let (p, ring, album) = player(&d);
    p.sync(Command::Play {
        album,
        index: 0,
        position: 0.0,
        playing: true,
    });
    wait(|| !p.snapshot().playing && p.snapshot().error.is_some());
    assert_eq!(ring.queued_frames(), 0);
}

#[test]
fn output_failure_pauses_and_clears_buffered_music() {
    let d = library(&["stereo480.flac"]);
    let (p, ring, album) = player(&d);
    p.sync(Command::Play {
        album,
        index: 0,
        position: 0.0,
        playing: true,
    });
    wait(|| ring.queued_frames() > 100);
    ring.report_output_error("Test output disconnected".into());
    wait(|| !p.snapshot().playing);
    assert_eq!(
        p.snapshot().error.as_deref(),
        Some("Test output disconnected")
    );
    assert_eq!(ring.queued_frames(), 0);
}

#[test]
fn external_app_launch_checkpoints_and_stops_music() {
    let d = library(&["stereo480.flac"]);
    std::fs::create_dir_all(d.path().join("System/device-info")).unwrap();
    std::fs::write(d.path().join("System/device-info/device-info"), b"fixture").unwrap();
    std::fs::write(d.path().join("System/device-info.sh"), b"fixture").unwrap();
    let (mut s, sink) = session(&d);
    press(&mut s, Btn::Start, 100);
    wait(|| sink.ring().queued_frames() > 100);
    s.app_mut().apply(Action::QuickMenu);
    for _ in 0..slot_ui::QuickRow::DeviceInfo.position() {
        s.app_mut().apply(Action::GbaDown(Btn::Down));
    }
    s.app_mut().apply(Action::GbaDown(Btn::A));
    assert!(s.app().device_info_requested());
    s.update(0.016);
    assert!(!s.app().music.snapshot.playing);
    assert!(MusicSettings::read(d.path()).resume.is_some());
    assert_eq!(sink.ring().sample_rate(), slot::audio::GBA_HZ);
}

#[test]
fn paused_startup_restore_and_unused_worker_do_not_clear_other_audio() {
    let d = library(&["stereo480.flac"]);
    let (p, ring, album) = player(&d);
    ring.push(&[1200; 512]);
    p.sync(Command::Play {
        album,
        index: 0,
        position: 0.5,
        playing: false,
    });
    assert_eq!(ring.queued_frames(), 256);
    p.sync(Command::Pause);
    assert_eq!(ring.queued_frames(), 256);
    drop(p);
    assert_eq!(ring.queued_frames(), 256);
}

#[test]
fn analysis_is_pre_volume_and_clears_on_seek() {
    let d = library(&["stereo480.flac"]);
    let (p, ring, album) = player(&d);
    p.sync(Command::Volume(0));
    p.sync(Command::Visible(true));
    p.sync(Command::Play {
        album,
        index: 0,
        position: 0.0,
        playing: true,
    });
    wait(|| {
        let mut silent = vec![0; ring.queued_frames() * 2];
        ring.fill(&mut silent);
        assert!(silent.iter().all(|sample| *sample == 0));
        p.snapshot().analysis.bars.iter().any(|bar| *bar > 0.1)
    });
    p.sync(Command::Pause);
    p.sync(Command::Seek(0.5));
    assert!(p.snapshot().analysis.bars.iter().all(|bar| *bar == 0.0));
    assert_eq!(ring.queued_frames(), 0);
}

#[test]
fn six_row_panel_handles_long_titles_and_separate_selected_playing_tracks() {
    let d = library(&["mono441.mp3"]);
    let mut album = slot_store::music::scan(d.path()).remove(0);
    album.title =
        "An unusually long album title with enough words to test clipping and fitting".into();
    let track = album.tracks[0].clone();
    album.tracks = (0..12)
        .map(|index| {
            let mut track = track.clone();
            track.path = album.path.join(format!("{index}.mp3"));
            track.number = Some(index + 1);
            track.title = format!(
                "Track {} with an unusually long title that must stay inside the title column",
                index + 1
            );
            track
        })
        .collect();
    let mut browser = Browser::default();
    browser.replace(vec![album.clone()]);
    browser.track = 8;
    browser.snapshot.album = Some(Arc::new(album));
    browser.snapshot.index = 4;
    browser.snapshot.playing = true;
    let rgba = panel_face(&browser);
    let at = ((227 * PANEL_W + 12) * 4) as usize;
    assert_eq!(&rgba[at..at + 4], &[48, 67, 52, 255]);
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/music-panel-long.png");
    image::save_buffer(&output, &rgba, PANEL_W, PANEL_H, image::ColorType::Rgba8).unwrap();
}
