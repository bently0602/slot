# SNES run-ahead audio research

Status: improvements implemented and host checks passed; H700 acceptance pending.
Date: 2026-10-14.

The user reports distorted SNES audio with both one- and two-frame run-ahead on
every game tried, on an Anbernic RG SP revision using H700. No specific game or
installed build was identified. The installed SD-card package remains untouched.

## Result

Audio starvation from extra run-ahead work is the leading hypothesis. Two
specific sources of unnecessary work are confirmed in Slot's Snes9x integration.
Neither is yet established as the cause of the reported handheld distortion.

The existing second-instance fix in task 019 already protects the audible core
from repeated state loads. Its real-core audio comparisons did not include
SNES; the SNES session integration test exercises a silent input/video ROM.

## Real-core comparison

`tests/research_snes_runahead.py` generates original 65816/SPC700 homebrew that
uploads a small program via the IPL protocol and enables deterministic DSP
noise. It uses no commercial game or BIOS assets. It loads private copies of
the actual host library, warms up 60 frames, and captures 180 frames. The
baseline contains 191924 interleaved i16 samples and 21665 distinct values.

Core: `target/desktop/snes9x_libretro.so`, built from libretro/snes9x commit
`fae2fea08f74180759ef540ee94259213f503480`, x86_64 Linux under WSL.

| Capture | Samples differing from baseline | Sample counts equal |
| --- | ---: | --- |
| Repeated normal run | 0 | Yes |
| Save every frame, without restore | 0 | Yes |
| Save and immediately restore | 0 | Yes |
| Single-instance run-ahead, 1 / 2 frames | 0 / 0 | Yes |
| Separate-instance run-ahead, 1 / 2 frames | 0 / 0 | Yes |
| Separate instance with video skipping and fast state context, 1 / 2 frames | 0 / 0 | Yes |

This rules out waveform corruption for this generated ordinary-SPC workload
on this host core. It does not cover BRR voice playback, echo, MSU-1 streaming,
special-chip games, game-specific input transitions, the Slot worker/resampler,
ALSA, or ARM device timing. The script drives libretro directly, not the full
Slot frontend. Its optional fast context is an experiment, not a production fix.

Run from the repository root in WSL/Linux:

```sh
python3 tests/research_snes_runahead.py
# Or supply a different Linux host core explicitly:
python3 tests/research_snes_runahead.py --core /path/to/snes9x_libretro.so
```

## Confirmed integration gaps

1. **Hidden frames still render.** `LibretroCore::set_frame_skip` only reports
   an artificial audio-buffer underrun through `SET_AUDIO_BUFFER_STATUS_CALLBACK`.
   This Snes9x core registers no such callback. Instead, its `retro_run` queries
   `GET_AUDIO_VIDEO_ENABLE` and enables rendering when that query is unsupported.
   Slot returns unsupported. Thus the primary's discarded picture and the
   secondary's intermediate pictures are still rendered and converted by Slot.
   In the experiment, the primary emitted 180 video callbacks in 180 captured
   frames with separate-instance run-ahead; answering the A/V query to disable
   primary video reduced that to zero without changing primary audio samples.

2. **Every speculative restore uses the normal restore path.** Snes9x checks
   `GET_SAVESTATE_CONTEXT`, then the older fast-savestate A/V bit. Slot supports
   neither. Snes9x therefore uses its normal path, including `S9xReset`, temporary
   copies of large memory blocks, and clearing the graphics buffer, for the
   speculative core on every frame. The fast path avoids some of that work.
   The experiment returned `RUNAHEAD_SAME_BINARY` for the optimized instances.

3. **Audio delivery waits for speculation.** In `Worker::run`, primary audio
   is taken before serialization, but resampling and `ring.push_blocking` happen
   after state restore and speculative frames. A slow speculative frame can
   delay already available audio. `Ring::fill` inserts zeroes when it runs short
   and then re-primes, which can produce audible discontinuities. This is a
   verified ordering/risk, not a measured device underrun.

The blank-screen homebrew's illustrative host averages were about 0.89/1.20 ms
per iteration at 1/2 frames, versus 0.70/1.00 ms with video skipping and fast
state context combined. Python callbacks, warmup, and the minimal graphics
workload make these unsuitable as handheld performance estimates or a reliable
percentage speedup. The useful evidence is unchanged PCM and eliminated hidden
video callbacks.

## Next validation and implementation direction

- Record the installed Slot/core build and affected game. Compare the same game
  scene with run-ahead off, one frame, and two frames on the handheld.
- Use `SLOT_TRACE=1` and inspect `slot: audio` logs: queue occupancy and cumulative
  `starved`/`dropped` counters. Measure primary emulation, save, secondary restore,
  speculative emulation, and time until the audio ring receives samples. Existing
  normal-speed logs do not give all these timings.
- Implement the libretro A/V query so hidden video is disabled while audible
  primary audio continues. Keep the existing status-callback fallback for cores
  that use it. An audio-discard flag is not permission to stop advancing audio
  state: the hard-disable bit has a separate, narrower contract.
- Add scoped run-ahead savestate context around transient speculative state
  operations, preserving normal context for persistent saves and rewind. Keep
  states portable between the separately loaded copies of the same core.
- Consider delivering primary audio before speculative work, with verification
  of pacing and ring behavior. Validate the affected game on hardware before
  claiming the distortion is fixed.

## Primary references

- [Libretro run-ahead guide](https://docs.libretro.com/guides/runahead/): clean
  savestates, why a second instance prevents buzzing, and additional CPU demand.
- [Pinned Snes9x libretro implementation](https://github.com/libretro/snes9x/blob/fae2fea08f74180759ef540ee94259213f503480/libretro/libretro.cpp):
  `retro_run` A/V query and `savestate_wants_fast_path`.
- [Pinned Snes9x restore implementation](https://github.com/libretro/snes9x/blob/fae2fea08f74180759ef540ee94259213f503480/snapshot.cpp):
  normal versus fast restore.
- [SNESdev SPC700 boot protocol](https://snes.nesdev.org/wiki/Booting_the_SPC700):
  homebrew upload handshake.

## Implementation follow-up

The libretro host now answers `GET_AUDIO_VIDEO_ENABLE`. `set_frame_skip` disables
video while keeping audio enabled, and callbacks from cores that ignore that flag
are discarded without framebuffer conversion. The existing audio-buffer status
callback remains available for cores that use it.

Only `unserialize` in a private speculative instance reports
`RUNAHEAD_SAME_BINARY`, with the old fast-savestate A/V bit as a compatibility
fallback. The context resets after successful or failed restores. Primary
serialization stays in normal context: its output is still safe to reuse for
rewind, persistent saves, resume, and another process. No transient fast-format
state is written to persistent storage.

The worker queues primary audio before serialization and speculative work.
Audio processing/backpressure time stays excluded from core execution timing.
Normal playback and fast-forward retain their previous output ordering.
Thumbnails use the actual displayed core: the speculative image during run-ahead,
or the primary image during fast-forward and rewind.

Validation:

- All 54 slot-retro unit tests pass; slot-retro strict all-target Clippy passes.
- Real Snes9x ordinary-SPC PCM comparison passes at zero, one, and two frames,
  including the repeated baseline and save/restore control: 191924 identical
  interleaved samples at each setting. The fixture is now part of
  `crates/slot/tests/run_ahead_audio.rs`, using Slot's actual libretro adapter.
- Real mGBA GB/GBA, gpSP GBA, and PCSX SPU waveform comparisons also pass with the
  updated adapter. No core is skipped in the explicit-path validation run.
- All 48 emulator tests pass across the grouped run and an isolated rerun.
  The grouped run had one previously documented host timing failure:
  `a_stalled_display_does_not_stall_the_worker` advanced 8 frames in its 200 ms
  window during concurrent builds. It passes in an isolated process. All nine
  run-ahead tests pass, including audio queued before secondary restore and
  thumbnails through normal/fast/normal speed transitions.
- Real SNES NTSC/PAL and interlace/multitap coverage checks hidden video stays
  unchanged while audio continues, followed by successful visible rendering.
- The real SNES session test passes saves, load/undo, rewind, resume, controller
  assignment, and run-ahead. All state-menu tests pass.

The optimized H700 ARM64 replacement executable is packaged separately from the SD
card installation at `target/slot-h700-snes-runahead-test.zip`, containing
`System/slot`, installation/rollback instructions, and a SHA256 checksum. It was
cross-built with the project's device profile and a glibc 2.31 target:

```sh
cargo zigbuild --profile device --target aarch64-unknown-linux-gnu.2.31 \
  --target-dir target/h700-runahead-native \
  -p slot --no-default-features --features device
```

ELF inspection confirms ARM aarch64 and `/lib/ld-linux-aarch64.so.1`. The binary
successfully runs `--init-root` and creates the expected SNES directory inside
an ARM64 Bullseye container. The archive's executable checksum and ZIP integrity
were verified. No installed SD-card files were replaced.

The changed slot Clippy targets also pass with `--no-deps` and the existing
Bluetooth type-complexity warning excluded. Full dependency strict lint still
reports unrelated manual-range checks in the existing PSX animation file.

Physical playback still needs comparison with run-ahead off,
one frame, and two frames. These checks establish the implementation behavior;
they do not establish that the reported handheld distortion is fixed.
