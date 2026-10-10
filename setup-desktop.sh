#!/usr/bin/env bash
# Ubuntu/Debian desktop setup, including Ubuntu in WSL. Run as your normal user.
set -euo pipefail

run=false
native_graphics=false
for option in "$@"; do
  case "$option" in
    --run) run=true ;;
    --native-graphics) native_graphics=true ;;
    -h|--help)
      printf 'Usage: bash setup-desktop.sh [--run] [--native-graphics]\n\nInstall prerequisites, build slot and mGBA, and prepare ./sdcard.\nAdd --run to launch the desktop app. Safe to rerun; existing content is kept.\nWSL launches use X11 and software rendering. --native-graphics keeps your normal graphics configuration.\n'
      exit 0
      ;;
    *) printf 'Unknown option: %s\nUse --help for usage.\n' "$option" >&2; exit 2 ;;
  esac
done

if [[ $(uname -s) != Linux ]] || ! command -v apt-get >/dev/null || ! command -v dpkg-query >/dev/null; then
  printf 'Run this script in Ubuntu/Debian, or Ubuntu through WSL on Windows.\n' >&2
  exit 1
fi
case "$(uname -m)" in
  x86_64) core_arch=x86-64 ;;
  aarch64) core_arch=aarch64 ;;
  *) printf 'Unsupported desktop architecture: %s\n' "$(uname -m)" >&2; exit 1 ;;
esac

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
cd "$repo_dir"
content_root="$repo_dir/sdcard"
build_dir="$repo_dir/target/desktop"
core="$build_dir/mgba_libretro.so"

# Install only missing packages, so subsequent runs need no sudo or apt update.
packages=(
  ca-certificates curl git file build-essential cmake pkg-config
  libasound2-dev libasound2-plugins libudev-dev libwayland-dev
  libxkbcommon-dev libxkbcommon-x11-dev libegl1 libegl-mesa0
  libgl1 libglx-mesa0 libgl1-mesa-dri
  libx11-6 libxcursor1 libxrandr2 libxi6
)
missing_packages=()
for package in "${packages[@]}"; do
  if [[ $(dpkg-query -W -f='${Status}' "$package" 2>/dev/null || true) != 'install ok installed' ]]; then
    missing_packages+=("$package")
  fi
done
if (( ${#missing_packages[@]} )); then
  apt_command=(env DEBIAN_FRONTEND=noninteractive apt-get)
  if (( $(id -u) != 0 )); then
    if ! command -v sudo >/dev/null; then
      printf 'Installing prerequisites requires sudo. Ask your administrator to install sudo first.\n' >&2
      exit 1
    fi
    apt_command=(sudo "${apt_command[@]}")
  fi
  printf 'Installing missing desktop prerequisites...\n'
  "${apt_command[@]}" update
  "${apt_command[@]}" install -y --no-install-recommends "${missing_packages[@]}"
else
  printf 'Desktop prerequisites are already installed.\n'
fi

# Read the repository pins rather than maintaining another copy of the versions.
rust_channel="$(sed -n 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain.toml)"
mgba_commit="$(sed -n 's/^  MGBA_COMMIT: \([0-9a-f]*\)[[:space:]]*$/\1/p' taskfile.yml)"
if [[ -z $rust_channel || ! $mgba_commit =~ ^[0-9a-f]{40}$ ]]; then
  printf 'Could not read the Rust or mGBA version pinned by this repository.\n' >&2
  exit 1
fi
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
host_triple="$(rustup run "$rust_channel" rustc -vV | sed -n 's/^host: //p')"
if [[ -z $host_triple ]]; then
  printf 'Could not determine the native Rust target.\n' >&2
  exit 1
fi

printf 'Building the desktop app for %s...\n' "$host_triple"
rustup run "$rust_channel" cargo build --release --locked -p slot \
  --target "$host_triple" --target-dir "$build_dir"
binary="$build_dir/$host_triple/release/slot"
"$binary" --init-root "$content_root"

# The stamp includes the pinned commit, flags and patch hashes. Check the binary
# architecture too, so an ARM64 device core is never reused on an x86 desktop.
expected_stamp="$(sh cores/mgba/build.sh stamp "$mgba_commit")"
if [[ -f $core && -f $core.meta ]] &&
   [[ $(cat "$core.meta") == "$expected_stamp" ]] &&
   file "$core" | grep -q "$core_arch"; then
  printf 'Desktop mGBA is already up to date.\n'
else
  printf 'Building desktop mGBA...\n'
  sh cores/mgba/build.sh build "$mgba_commit" "$repo_dir/target/mgba-host" "$core"
fi
if ! file "$core" | grep -q "$core_arch"; then
  printf 'The mGBA core does not match the desktop architecture.\n' >&2
  exit 1
fi
if ! cmp -s "$core" "$content_root/System/mgba_libretro.so"; then
  cp "$core" "$content_root/System/mgba_libretro.so"
fi
if ! cmp -s "$core.meta" "$content_root/System/mgba_libretro.so.meta"; then
  cp "$core.meta" "$content_root/System/mgba_libretro.so.meta"
fi

# CPAL uses ALSA on Linux; WSLg exposes PulseAudio. Keep this configuration local
# to slot, and respect an explicitly supplied ALSA_CONFIG_PATH.
if [[ -n ${WSL_DISTRO_NAME:-} && -z ${ALSA_CONFIG_PATH:-} ]]; then
  audio_config="$build_dir/wsl-asound.conf"
  if [[ ! -f $audio_config ]]; then
    cat > "$audio_config" <<'ALSA'
</usr/share/alsa/alsa.conf>
pcm.!default {
  type pulse
}
ctl.!default {
  type pulse
}
ALSA
  fi
  export ALSA_CONFIG_PATH="$audio_config"
fi

printf '\nDesktop setup is ready at %s\n' "$content_root"
printf 'Put unzipped ROMs in Games/GBA, Games/GB or Games/GBC; use the mGBA core.\n'
printf 'Keys: arrows, Z/X (A/B), A/S (shoulders), Enter (Start), right Shift (Select), Tab (Menu).\n'
if $run; then
  if [[ -z ${DISPLAY:-} && -z ${WAYLAND_DISPLAY:-} ]]; then
    printf 'No graphical display is available. Open a WSLg/desktop terminal and run this script with --run again.\n' >&2
    exit 1
  fi
  # WSLg's Wayland/GPU path can fail or crash Weston on some Windows hosts.
  # Scope the compatibility settings to this app; leave the user's shell alone.
  if [[ -n ${WSL_DISTRO_NAME:-} ]] && ! $native_graphics; then
    if [[ -z ${DISPLAY:-} ]]; then
      printf 'WSL compatibility graphics needs an X11 DISPLAY. Start this script in a WSLg terminal, or try --native-graphics.\n' >&2
      exit 1
    fi
    unset WAYLAND_DISPLAY WAYLAND_SOCKET MESA_LOADER_DRIVER_OVERRIDE
    export LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe
    printf 'WSL compatibility graphics: X11 with software rendering.\n'
  fi
  export SLOT_ROOT="$content_root"
  exec "$binary"
fi
printf 'Launch with: bash "%s/setup-desktop.sh" --run\n' "$repo_dir"
