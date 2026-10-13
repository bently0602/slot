#!/usr/bin/env bash
# Verify device compilation routing without compiling or starting containers.
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
fixture="$(mktemp -d)"
trap 'rm -rf -- "$fixture"' EXIT
mkdir -p "$fixture/bin"
cp "$repo_dir/taskfile.yml" "$fixture/taskfile.yml"
export DEVICE_CONTAINER_TEST_LOG="$fixture/calls"
export PATH="$fixture/bin:$PATH"

cat > "$fixture/bin/uname" <<'MOCK'
#!/bin/sh
case "$1" in -s) echo Linux ;; -m) echo "$DEVICE_CONTAINER_TEST_ARCH" ;; esac
MOCK
cat > "$fixture/bin/docker" <<'MOCK'
#!/bin/sh
printf 'docker platform=%s %s\n' "${DOCKER_DEFAULT_PLATFORM:-unset}" "$*" >> "$DEVICE_CONTAINER_TEST_LOG"
MOCK
cat > "$fixture/bin/cargo" <<'MOCK'
#!/bin/sh
printf 'cargo %s\n' "$*" >> "$DEVICE_CONTAINER_TEST_LOG"
MOCK
cat > "$fixture/bin/file" <<'MOCK'
#!/bin/sh
printf '%s: ELF 64-bit, ARM aarch64\n' "$1"
MOCK
chmod +x "$fixture/bin/"*
cd "$fixture"
export DOCKER_DEFAULT_PLATFORM=linux/arm64

# Native ARM64 retains the local build unless explicitly forced into Docker.
export DEVICE_CONTAINER_TEST_ARCH=aarch64
unset SLOT_DEVICE_DOCKER
task build:device > "$fixture/task.log" 2>&1
grep -Fxq 'cargo build --profile device --target-dir target-device -p slot --no-default-features --features device' "$DEVICE_CONTAINER_TEST_LOG"
! grep -q '^docker ' "$DEVICE_CONTAINER_TEST_LOG"

: > "$DEVICE_CONTAINER_TEST_LOG"
export SLOT_DEVICE_DOCKER=1
task build:device > "$fixture/task.log" 2>&1
grep -Fq 'docker platform=linux/arm64 run --rm' "$DEVICE_CONTAINER_TEST_LOG"
grep -Fq 'rust:1-bullseye sh -c cargo build' "$DEVICE_CONTAINER_TEST_LOG"
! grep -q '^cargo ' "$DEVICE_CONTAINER_TEST_LOG"

# x86 hosts continue using Docker without the override.
: > "$DEVICE_CONTAINER_TEST_LOG"
export DEVICE_CONTAINER_TEST_ARCH=x86_64
unset SLOT_DEVICE_DOCKER
task build:device > "$fixture/task.log" 2>&1
grep -Fq 'docker platform=linux/arm64 run --rm' "$DEVICE_CONTAINER_TEST_LOG"
! grep -q '^cargo ' "$DEVICE_CONTAINER_TEST_LOG"
echo 'Device container routing tests passed (native ARM64, forced ARM64, x86).'
