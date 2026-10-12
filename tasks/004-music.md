# Music shelf with album flip and retro visualizers

Status: implemented; physical handheld validation pending. Authorized by the approved feature plan in the chat.

Implement folder-defined MP3/FLAC albums under Games/Music, background scanning,
tag/filename ordering, local/embedded artwork and bounded artwork caching.
Provide square coverflow, a flipping six-row track panel, transport controls,
shuffle/repeat, seeking, and compact/full-screen retro bars and waveform.
Music continues while browsing and stops before game/system-app launches.
Persist playback and settings; normal boot restores paused. Integrate headphone
removal, configurable closed-lid playback, sleep, volume/routes and HDMI restart.

Validate library, real decoding fixtures, transport/queues, visualizer analysis,
rendering and lifecycle behavior. Run formatting, workspace checks and host/device
builds. Document any unavailable physical hardware checks. Preserve existing work.

All five implementation stages are complete. Setup instructions and release notes
are updated. `MUSIC.md` describes the worker, buffering, artwork, UI, persistence
and lifecycle integration and records validation results and limitations.

Validated 28 music checks, a 185-test frontend regression subset, desktop and
optimized ARM64 builds, QEMU device initialization, strict workspace library/bin
lint, formatting and setup smoke tests. Separate storage/UI and Linux graphics
runs passed 434 and 26 checks. The wider Linux-compatible workspace run passed
1,406 tests; its fixture/audio/timing failures and the existing macOS-only
graphics test compilation restriction are recorded in `MUSIC.md`. The timing
failure passed an isolated recheck. No device was available for physical playback,
performance, lid or speaker/headphone/HDMI routing checks.
