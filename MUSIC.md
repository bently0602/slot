# Music implementation

Music is a separate shelf kind, not an emulator platform or cartridge. Library
identity uses paths relative to the content root; no absolute host paths are saved.
`slot-store::music` defines albums, tracks, artwork sources, settings and resume
state. A scanner thread recursively discovers direct-folder albums and reads
Symphonia metadata. Refreshes preserve selection and do not touch playback.

`Session` owns the music player independently of the current screen. Its worker
owns decoding, the album queue/history, transport, checkpoints and analysis.
It requests 48 kHz from the existing sink and uses Rubato sinc resampling to the
sink's actual rate. Mono is duplicated to stereo; multichannel files are rejected.
Pending writes retain their unwritten tail. Pause/seek clear queued samples before
decoding resumes; position subtracts queued frames from submitted audio. An output
that stops draining for three seconds pauses with an error. Game/system-app launch
stops the worker's transport and clears music before restoring game audio.

Analysis uses pre-volume floating-point samples and excludes menu sound effects.
RealFFT analyzes Hann-windowed 2,048-sample windows into 32 logarithmic bands, with
smoothing and falling peaks. Bounded snapshots are timestamped against the audible
buffer position. Analysis runs only for a visible panel/visualizer, at approximately
30 Hz; pause fades the previous snapshot. Screen-off playback disables analysis.

`music_ui` reuses the game shelf's coverflow pose and implements the vertical-axis
flip/expansion, track panel, footer and both visualizers. Artwork loads on a bounded
worker queue; up to nine cover textures are retained/reused. Cached JPEG/PNG covers
are keyed by source path/size/modification time. Folder revisions invalidate live
covers on rescan. Image dimensions, allocation and file size are bounded; unreadable
covers fall back to generated placeholders.

`Config/music.json` is replaced atomically at transport/settings changes, sleep,
shutdown, display checkpoints and every 15 seconds of playback. Ordinary boot
restores paused. The device's explicit `SLOT_MUSIC_DISPLAY_RESUME` restart marker
is consumed immediately and preserves playing/paused status only for display
restarts. Power-button sleep wins over music/docking lid exceptions.

## Delivery stages

1. Library/settings: folder discovery, metadata/artwork priorities, natural sorting,
   relative identities and atomic settings.
2. Playback: decoder/resampler, queue/history, cancellable audio buffering,
   transport, checkpoints and existing output routing.
3. Browsing: Music shelf kind, square coverflow, flip panel, menu and scoped controls.
4. Visualizers: FFT bands/peaks, waveform, compact/full-screen rendering and visibility.
5. Lifecycle/docs: lid/sleep/headphones, game/external-app stop, HDMI restart and setup.

## Validation

`slot-store/tests/music.rs` covers nested/root albums, corrupt/missing tags,
natural ordering, duplicate names, local/embedded artwork and unsafe/corrupt settings.
`slot/tests/music.rs` uses generated MP3/FLAC fixtures for mono/stereo, VBR MP3,
22.05/44.1/48/96 kHz and 24-bit FLAC; it covers resampling, transport/backpressure,
queue modes, failed albums, tone/silence analysis, control isolation, panel/visualizer
geometry, startup/display resume and lid/power transitions. Fixtures contain only
original generated tones; provenance is recorded alongside them.

Physical handheld validation remains required: sustained playback while browsing
and visualizing, controller responsiveness, memory behavior, lid-closed playback,
headphone/speaker/HDMI output routing, sleep/shutdown and actual HDMI reconnection.
These cannot be established by simulated sinks or render geometry tests.

### Results in this environment

- 28 focused music checks passed: 21 integration tests, five library/settings
  tests, and decoder/artwork unit tests. The final Slot regression subset passed
  185 tests (music, application controls, menus, docking and library unit tests).
  The separate storage/UI run passed 434 tests, and 26 Linux graphics checks passed.
- Desktop debug and optimized ARM64 device builds passed. The ARM64 executable
  was verified as an aarch64 ELF and executed under QEMU for a root-initialization
  smoke check; both host and device binaries created `Games/Music`.
- Workspace library/binary Clippy passed with warnings denied. Rust formatting,
  whitespace checks, script syntax and device-setup smoke tests passed.
- The Linux-compatible workspace run passed 1,406 tests with 43 failures and three
  ignored tests. Failures included unavailable mGBA/gpSP fixture libraries, a
  missing-core link reload expectation, unavailable desktop audio and one timing
  test under concurrent builds. The timing test passed its isolated recheck.
- Unfiltered workspace tests cannot compile on Linux because the existing graphics
  pipeline tests import macOS-only `HeadlessSurface`. GPU render checks and
  physical handheld tests were unavailable. The track panels were inspected as
  software-rendered PNGs; cover/flip and visualizer geometry were checked in tests.

Builds and logs were placed under `D:/CodexBuild/slot-b-music` after the C: build
cache filled its drive. No handheld deployment was performed.
