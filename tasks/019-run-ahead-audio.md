# Investigate run-ahead audio distortion

Status: cause reproduced and fix implemented; handheld acceptance pending.
Requested by the user, who also authorized tests and a fix for a confirmed cause.

The user reports incorrect audio with one-frame run-ahead across cores, including
GB, while other firmware works on the same device. Identify the cause before
changing playback behavior.

## Investigation

- Compare audio produced with run-ahead off, one frame, and two frames using real
  cores and a deterministic sound-producing ROM, independently of device pacing.
- Check whether speculative frames or state restore alter sample counts,
  waveform continuity, pending audio, or converter/filter history.
- Review the shared worker's audio delivery, frame skipping, display pacing,
  and underrun recovery. Distinguish waveform corruption from missed deadlines.
- Record measured findings and reproduction instructions. Existing mock tests
  alone do not establish real-core audio correctness.

## Follow-up implementation

Add a regression reproduction and the smallest fix supported by the results.
Verify normal playback, run-ahead frame advancement,
pause, fast-forward, rewind, and state-restore behavior as appropriate.
Preserve unrelated changes already present in the checkout.

## Acceptance

Document a reproducible cause or the remaining evidence needed from the handheld.
Do not claim hardware audio is fixed without device validation. Do not overwrite
the installed SD-card package during investigation.

## Findings and implementation

The original single-instance run-ahead restores a state into the audible core
every frame. This changes the audio output even in an unpaced, headless test:
a save-and-immediate-restore control with no speculative frames fails too.
The defect is therefore reproducible independently of ALSA, speaker routing,
resampling in Slot, or missed handheld deadlines.

Generated GB and GBA ROMs play a continuous square wave without game assets.
After 60 warmup frames, capture 180 real frames and compare all interleaved i16
samples against an independently repeated, deterministic baseline.

| Core and system | Baseline samples | Old 1-frame samples | Differing shared samples | Fixed 1/2-frame differences |
| --- | ---: | ---: | ---: | ---: |
| mGBA / GB | 790528 | 783360 | 770774 | 0 |
| mGBA / GBA | 395008 | 395008 | 215278 | 0 |
| gpSP / GBA (host interpreter) | 395010 | 394920 | 99568 | 0 |
| PCSX ReARMed / PSX (host interpreter, SPU tone) | 264600 | 264600 | 0 | 0 |

The new worker keeps the primary core advancing normally and synchronizes a
separate loaded instance for speculative video. Only primary audio reaches the
output ring. Normal audio delivery and frame-budget calculation are retained.
The temporary copied core library prevents dlopen from sharing the primary's
global variables. The copy has a private temporary save directory, inherits core
options, BIOS directory, video canvas and controller device assignments, and is
removed when the instance closes. Option/controller changes recreate it. Off,
link activation, or an instance/state failure releases it. Failures log a reason
and disable run-ahead for the session rather than restoring the audible core.

Reproduction and regression commands (Ubuntu/WSL, repository root):

```bash
SLOT_AUDIO_TEST_CORE="$PWD/target/mgba-host/build/mgba_libretro.so" \
SLOT_GPSP_AUDIO_TEST_CORE="$PWD/target/gpsp-audio-host/gpsp-7b2dd37394e2822110cacef321e9bb291a305b77/gpsp_libretro.so" \
cargo test -p slot --no-default-features --test run_ahead_audio -- --nocapture --test-threads=1
```

The regression suite prefers the host mGBA library in `target/desktop` if an
explicit path is omitted. Set `SLOT_AUDIO_TEST_SINGLE_INSTANCE=1` on the same
command to reproduce the old behavior; the tone assertions then intentionally
fail. The old-path gpSP reproduction was also executed and failed as expected.

PSX follow-up: a generated MIPS executable uploads a looping ADPCM square wave
to the SPU, using PCSX ReARMed's HLE BIOS and host interpreter. Both old and new
paths produce all 264600 interleaved samples unchanged over 180 captured frames,
including the restore-only control and one/two-frame run-ahead. The SPU tone is
checked for nonzero output and the normal baseline is repeated for determinism.
This does not reproduce the earlier corruption in PSX; do not infer that PSX
games' CDDA/XA audio or device timing are covered by the SPU-tone comparison.
The real multi-disc fixture additionally checks that the speculative instance
restores the mounted disc while preserving the primary picture and memory card.

Validation includes real mGBA/gpSP waveform comparisons, independent library
ownership and video isolation, worker regression coverage for never restoring
the primary, initialization failure fallback, and existing run-ahead tests.
The real Snes9x session integration test also passes with run-ahead, controller
multitap changes, state save/load/undo, rewind and resume. Audio ring tests and
all state-menu tests pass. slot-retro's 52 unit tests and strict lint pass; the
device-feature build check passes.

## Remaining validation

Physical speaker/headphone playback and frame costs still need checking on the
handheld, especially ARM gpSP dynarec, SNES and PS1. PS1 CDDA/XA playback and ARM
dynarec are not tested by the host SPU fixture. A second core uses extra
memory and startup work; heavy games can still exceed the device's frame budget.
The SD-card package has not been replaced by this investigation.

Grouped emulator tests had intermittent timing failures on this WSL host;
the failing fast-frame and stalled-display checks pass in individual processes.
All 46 emulator tests were then run in separate processes and passed (46/46).
Full-slot strict lint also reports pre-existing warnings in Bluetooth and
system_app; the affected run-ahead targets pass with the unrelated Bluetooth
type-complexity warning excluded. No unrelated lint code was changed.
