# Battery and software standby

Slot preserves its existing software standby and BaseOS shutdown commands. It
does not write to `/sys/power/state`, change Super Standby configuration, or claim
suspend-to-RAM power savings. A software timer cannot run while the kernel is
suspended. BaseOS's documented SP deep-sleep configuration excludes lid wake;
Power wakes it. A future kernel-suspend mode needs separately tested input,
audio/display recovery and an RTC wake alarm for timed shutdown.

Settings live in `Config/slot.state`: `sleep_shutdown` is minutes (1, 3, 5, 10,
30; 0 is Never), and `shutdown_charging` is 0/1. Missing or invalid new values
use three minutes and Off without resetting other settings. The timer begins on
actual standby, so closed-lid music and HDMI keep-awake exceptions still apply.
Charging/Full resets the elapsed standby time while Shutdown While Charging is
Off. Percent/status are read from the battery's sysfs capacity/status files;
unknown status is reported without inventing a charging state.

The early checkpoint triggers at 10%, resets above 15% or while Charging/Full,
and is invalidated by each game launch. It waits for a seated game without an
active link. Writes settle before the checkpoint; battery RAM is written before
the resume, so a missing serializer or failed resume write can still preserve
battery RAM. Neither untrusted data nor failed writes receive success feedback.
Failures retry after 30 seconds. At 5%, the link ends before the final checkpoint;
failure is logged and shutdown proceeds because power may be exhausted. A
standby game was already saved when it entered standby.

## Hardware acceptance

- Confirm percentage against the firmware gauge, and verify Charging/Full/
  Discharging while attaching and removing power on the shelf and in the menu.
  Inspect unknown/missing telemetry rather than treating it as an empty battery.
- Play a disposable test game across 10%: verify the low-battery message, resume
  and `.sav` on the card, and no extra manual slot/history entries. Eject and
  relaunch below 10%; verify a new checkpoint.
- With a disposable test card, make the resume destination unwritable. Verify
  failure feedback and `slot.log`; restore access and check the retry after 30
  seconds. Verify battery RAM survives a resume-only write failure.
- Cross 5% unplugged: verify final save, normal BaseOS shutdown and next-boot
  resume. Confirm Charging/Full suppress this action at a low reported percent.
- Set one minute, close the lid or tap Power, and verify shutdown after the
  delay. Wake before it expires, sleep again, and verify a fresh full delay.
  Restart and confirm the selected setting persists. Check Never as well.
- Connect power just before the standby deadline with Shutdown While Charging
  Off; verify it stays asleep without powering off. Unplug, wait a fresh delay,
  and confirm shutdown. Repeat with the option On and with Full status.
- Verify closed-lid music and docked keep-awake continue playing past the chosen
  delay; pausing music or undocking into standby starts the normal timer.
- Repeat sleep/wake and charging checks on each supported device. Physical
  discharge, timing accuracy and real card durability remain hardware checks.
