# Battery and standby polish

Requested by the user. Status: in progress.

Add Menu → Battery with live percentage/charging status, persisted standby
shutdown choices (1, 3, 5, 10, 30 minutes, Never; default 3), and Shutdown While
Charging (default Off). Preserve current software standby, lid wake, closed-lid
music and docked play. Kernel suspend is a separate hardware integration: a
process timer cannot expire while suspended and stock SP deep sleep requires
Power to wake.

At 10% or less, save the active game's resume and battery save before showing
success feedback. Avoid repeated saves on every poll, but save newly launched
games too. Reset after charging or recovery above 15%; retry failed writes after
30 seconds and show failure feedback. Keep the existing final save and shutdown
at 5%, including when shutdown delay is Never. Charging/Full suppress low-battery
actions. Defer the early save during link and core-load transitions. Validate
persistence, thresholds, save failures, charging changes, timeout and wake behavior.
