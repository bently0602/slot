# Fix Docker access in the release workflow

Make the release workflow build Device Info with Docker while keeping Slot and
the emulator cores compatible with BaseOS's Bullseye runtime.

Status: complete. Requested by the user.

Run Task on the ARM64 GitHub runner so Docker bind mounts use host workspace
paths. Set `SLOT_DEVICE_DOCKER=1` to force the existing Bullseye device containers
even on native ARM64. Device Info continues using its own Bullseye SDK container.
Check Docker before building and validate the packaged Device Info binary,
private SDL runtime and corresponding source archive before publishing.
The CI build runs Task with sudo so assembly can write root-owned directories
created by the Docker builds on the Linux runner.

Validation passed: Actionlint (including ShellCheck), device-container routing
tests for native ARM64/default, native ARM64/forced and x86, setup-device smoke
tests, and a complete local ARM64 distribution assembled with the release's
container override. Existing core build caches were reused. All workflow
shipment checks passed against that bundle. Confirmed sudo preserves the two
build environment variables and can access Docker. GitHub-hosted execution and
publishing were not triggered; these changes remain local.
