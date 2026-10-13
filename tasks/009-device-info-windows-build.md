# Fix Device Info builds from Windows/WSL

Resolve the silent `app:device-info:device` failure reported while running
`setup-device.sh`. Preserve build caches and make formatting failures actionable.

Status: complete. Requested by the user through a setup failure report.

Reproduced the failure in Ubuntu through WSL: all four Go files had CRLF line
endings, so `test -z "$(gofmt -l ./*.go)"` exited without displaying the filenames.
Git now requires LF for Go sources, this checkout has been normalized, and
formatting failures print the affected files and repair command. BUILDING.md
documents the repair for older Windows checkouts.

Validation passed in Ubuntu through WSL: shell syntax check, setup-device smoke
tests, a CRLF formatting-failure probe, Device Info's ARM64 Go tests/build and
packaged headless run, and the complete `bash setup-device.sh` workflow. The
resulting SD2 bundle is in `dist-device`. No handheld deployment was performed.
