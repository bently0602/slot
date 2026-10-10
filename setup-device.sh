#!/usr/bin/env bash
# Ubuntu/Debian hardware setup, including Ubuntu in WSL. Run as your normal user.
set -euo pipefail

for option in "$@"; do
  case "$option" in
    -h|--help)
      cat <<'HELP'
Usage: bash setup-device.sh

Install missing prerequisites and build the RG SP/BaseOS SD2 bundle in ./dist-device.
Safe to rerun: builds are cached and existing games, BIOS, saves and settings are kept.
On x86_64, start Docker Desktop with Linux containers and WSL integration first,
or provide a working Linux Docker daemon with ARM64 emulation.
This script prepares files; it does not copy to a device or reflash an SD card.
HELP
      exit 0
      ;;
    *) printf 'Unknown option: %s\nUse --help for usage.\n' "$option" >&2; exit 2 ;;
  esac
done

fail() { printf '%s\n' "$*" >&2; exit 1; }
if [[ $(uname -s) != Linux ]] || ! command -v apt-get >/dev/null || ! command -v dpkg-query >/dev/null; then
  fail 'Run this script in Ubuntu/Debian, or Ubuntu through WSL on Windows.'
fi
case "$(uname -m)" in
  x86_64) task_arch=amd64; use_docker=true ;;
  aarch64) task_arch=arm64; use_docker=false ;;
  *) fail "Unsupported build architecture: $(uname -m)" ;;
esac

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
cd "$repo_dir"
[[ -f taskfile.yml && -f rust-toolchain.toml ]] || fail 'Keep setup-device.sh in the root of the Slot repository.'

if "$use_docker"; then
  command -v docker >/dev/null || fail 'Docker is missing. On Windows, install Docker Desktop, select Linux containers and enable Settings > Resources > WSL Integration for Ubuntu, then rerun this script.'
  docker info >/dev/null 2>&1 || fail 'Docker is not ready. Start Docker Desktop and enable WSL integration for this Ubuntu distro, or start your Linux Docker daemon, then rerun this script.'
  export DOCKER_DEFAULT_PLATFORM=linux/arm64
fi

# Subsequent runs need neither sudo nor apt update when packages are installed.
packages=(ca-certificates curl git file tar gzip coreutils)
if ! "$use_docker"; then
  packages+=(build-essential cmake pkg-config zlib1g-dev)
fi
missing_packages=()
for package in "${packages[@]}"; do
  if [[ $(dpkg-query -W -f='${Status}' "$package" 2>/dev/null || true) != 'install ok installed' ]]; then
    missing_packages+=("$package")
  fi
done
if (( ${#missing_packages[@]} )); then
  apt_command=(env DEBIAN_FRONTEND=noninteractive apt-get)
  if (( $(id -u) != 0 )); then
    command -v sudo >/dev/null || fail 'Installing missing prerequisites requires sudo. Install sudo or ask your administrator to install the listed packages.'
    apt_command=(sudo "${apt_command[@]}")
  fi
  printf 'Installing missing device prerequisites: %s\n' "${missing_packages[*]}"
  "${apt_command[@]}" update
  "${apt_command[@]}" install -y --no-install-recommends "${missing_packages[@]}"
else
  printf 'Device prerequisites are already installed.\n'
fi

# Install Go Task locally if absent (or if `task` is Taskwarrior), without Go/sudo.
setup_dir="$repo_dir/target/setup-device"
export PATH="$setup_dir/bin:$PATH"
task_help="$(task --help 2>&1 || true)"
task_version="$(task --version 2>/dev/null || true)"
if [[ $task_help != *--taskfile* || ! $task_version =~ (^|[[:space:]])v?3\. ]]; then
  task_release=3.54.0
  archive="task_linux_${task_arch}.tar.gz"
  release_url="https://github.com/go-task/task/releases/download/v${task_release}"
  mkdir -p "$setup_dir/downloads" "$setup_dir/bin"
  printf 'Installing Go Task %s locally...\n' "$task_release"
  curl --proto '=https' --tlsv1.2 -fSL --retry 3 -o "$setup_dir/downloads/$archive" "$release_url/$archive"
  curl --proto '=https' --tlsv1.2 -fsSL --retry 3 -o "$setup_dir/downloads/task_checksums.txt" "$release_url/task_checksums.txt"
  expected_sha="$(awk -v archive="$archive" '$2 == archive {print $1}' "$setup_dir/downloads/task_checksums.txt")"
  [[ $expected_sha =~ ^[0-9a-f]{64}$ ]] || fail 'Could not read the official Go Task checksum.'
  actual_sha="$(sha256sum "$setup_dir/downloads/$archive")"
  [[ ${actual_sha%% *} == "$expected_sha" ]] || fail 'Go Task download failed checksum verification. Rerun to download it again.'
  tar -xzf "$setup_dir/downloads/$archive" -C "$setup_dir/bin" task
  chmod +x "$setup_dir/bin/task"
  hash -r
fi

if "$use_docker"; then
  printf 'Checking ARM64 container support (the first run may download the Rust image)...\n'
  if ! container_arch="$(docker run --rm --platform linux/arm64 rust:1-bullseye uname -m)"; then
    fail 'Could not run the ARM64 Rust container. Check Docker networking and ARM64 emulation. Docker Desktop must use Linux containers.'
  fi
  [[ $container_arch == aarch64 ]] || fail "Expected an ARM64 container, received: $container_arch"
else
  # Native ARM64 builds run directly, so Rust must be installed on the host.
  rust_channel="$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain.toml)"
  [[ -n $rust_channel ]] || fail 'Could not read the pinned Rust version.'
  export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
  if ! command -v rustup >/dev/null; then
    printf 'Installing Rustup...\n'
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs |
      sh -s -- -y --profile minimal --default-toolchain none --no-modify-path
  fi
  if ! rustup run "$rust_channel" rustc --version >/dev/null 2>&1 ||
     ! rustup run "$rust_channel" rustfmt --version >/dev/null 2>&1 ||
     ! rustup run "$rust_channel" cargo-clippy --version >/dev/null 2>&1; then
    rustup toolchain install "$rust_channel" --profile minimal --component rustfmt --component clippy
  fi
fi

# Task recreates OUT, so use a generated staging folder and merge only after success.
# Keep this path relative and fixed: Task interpolates it into shell commands.
staging=target/device-bundle
printf 'Building the device app and mGBA, gpSP and PlayStation cores...\n'
task dist:device "OUT=$staging"
for binary in slot mgba_libretro.so gpsp_libretro.so pcsx_rearmed_libretro.so; do
  [[ -f "$staging/System/$binary" ]] || fail "Build did not produce System/$binary."
  binary_info="$(file "$staging/System/$binary")"
  [[ $binary_info == *aarch64* ]] || fail "Build produced a non-ARM64 binary: $binary_info"
done
mkdir -p dist-device
cp -R "$staging/." dist-device/

cat <<READY

Device setup is ready at $repo_dir/dist-device

1. Power off the handheld and connect its SD2 card to your computer.
2. Merge dist-device/System/ into SD2's System/ folder, replacing software files.
   Keep Games, BIOS, Config, Saves and States. Leave BaseOS on SD1 in place.
3. Safely eject the card, put it back and start Slot.

For docking tests, use Menu > Video Out: Output = Auto,
Keep Awake Docked = On, and Audio Output = Auto.
Start a game handheld, connect the hub/controller and HDMI, then close the lid.
Check TV video, audio and controls. Unplug HDMI and open the lid to resume handheld.
If something fails, check slot.log in the root of SD2.
READY
