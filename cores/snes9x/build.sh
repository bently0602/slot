#!/bin/sh
set -eu

usage() {
    echo "usage: $0 stamp COMMIT host|device | build COMMIT WORKDIR OUT host|device" >&2
    exit 2
}

configure() {
    platform=unix ext=so
    case "$1" in
        device) arch=aarch64 ;;
        host)
            arch=$(uname -m)
            case "$arch" in aarch64|arm64|x86_64) ;; *) usage ;; esac
            case "$(uname -s)" in
                Darwin) platform=osx; ext=dylib ;;
                Linux) ;;
                *) usage ;;
            esac
            ;;
        *) usage ;;
    esac
}

stamp() {
    configure "$2"
    printf 'commit=%s\narch=%s\nrecipe=make -C libretro platform=%s\n' "$1" "$arch" "$platform"
}

case "${1:-}" in
    stamp) [ "$#" -eq 3 ] || usage; stamp "$2" "$3" ;;
    build)
        [ "$#" -eq 5 ] || usage
        commit=$2 work=$3 out=$4 mode=$5
        expected=$(stamp "$commit" "$mode")
        if [ -f "$out" ] && [ -f "$out.meta" ] && [ -f "$out.source.tar.gz" ] && [ "$(cat "$out.meta")" = "$expected" ]; then
            echo "Snes9x is already up to date."
            exit 0
        fi
        if [ "$mode" = device ] && [ "$(uname -m)" != aarch64 ]; then
            echo "Build the device core in the ARM64 device container." >&2
            exit 1
        fi
        src="$work/snes9x-$commit"
        mkdir -p "$work" "$(dirname "$out")"
        if [ ! -d "$src/.git" ]; then
            git init "$src"
        fi
        src_abs=$(cd "$src" && pwd -P)
        if ! git -C "$src" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
            git config --global --add safe.directory "$src_abs"
        fi
        if ! git -C "$src" remote get-url origin >/dev/null 2>&1; then
            git -C "$src" remote add origin https://github.com/libretro/snes9x.git
        fi
        if ! git -C "$src" cat-file -e "$commit^{commit}" 2>/dev/null; then
            git -C "$src" fetch --depth 1 origin "$commit"
        fi
        git -C "$src" checkout --detach "$commit"
        configure "$mode"
        case "$(uname -s)" in Darwin) jobs=$(sysctl -n hw.ncpu) ;; *) jobs=$(getconf _NPROCESSORS_ONLN) ;; esac
        make -C "$src/libretro" platform="$platform" -j"$jobs"
        cp "$src/libretro/snes9x_libretro.$ext" "$out"
        git -C "$src" archive --format=tar --prefix="snes9x-$commit/" "$commit" | gzip > "$out.source.tar.gz"
        printf '%s\n' "$expected" > "$out.meta"
        ;;
    *) usage ;;
esac
