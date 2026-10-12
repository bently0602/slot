#!/bin/sh
set -eu
case "$APP_OUTPUT" in
    /src/target/desktop/device-info|/src/target-device/device-info) ;;
    *) echo "Unexpected app output directory: $APP_OUTPUT" >&2; exit 2 ;;
esac
mkdir -p "$APP_OUTPUT/lib" "$APP_OUTPUT/licenses"
# Go module-cache licenses are read-only; make prior output replaceable on
# Windows bind mounts as well as on Linux.
find "$APP_OUTPUT/licenses" -type f -exec chmod u+w {} +
find "$APP_OUTPUT/lib" -maxdepth 1 -type f -name '*.so*' -delete
if [ "$APP_MODE" = host ]; then
    # Use Debian's desktop SDL in the preview; the private Mali SDL is for H700.
    unset PKG_CONFIG_PATH
else
    # go-sdl2's companion bindings ask pkg-config only for sdl2, so their
    # headers must also be visible under the private SDL include directory.
    cp /usr/include/SDL2/SDL_image.h /usr/include/SDL2/SDL_ttf.h \
        /usr/include/SDL2/SDL2_*.h /opt/slot-sdl/include/SDL2/
    export LD_LIBRARY_PATH="/opt/slot-sdl/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
go mod download
test -z "$(gofmt -l ./*.go)"
go test -tags nonextuifonts ./...
go build -buildvcs=false -tags nonextuifonts -ldflags='-s -w' -o "$APP_OUTPUT/device-info" .

# Dereference every library: SD cards can use FAT, which has no symlinks.
# Keep the OS's glibc, loader and GPU drivers; bundle the app's other dependencies.
packages="$(mktemp)"
trap 'rm -f "$packages"' EXIT
if [ "${GOARCH:-}" = arm64 ] && [ "$(uname -m)" = x86_64 ]; then
    resolved="$(LD_LIBRARY_PATH=/opt/slot-sdl/lib /lib/ld-linux-aarch64.so.1 --list "$APP_OUTPUT/device-info")"
else
    resolved="$(LD_LIBRARY_PATH="${PKG_CONFIG_PATH:+/opt/slot-sdl/lib:}" ldd "$APP_OUTPUT/device-info")"
fi
if printf '%s\n' "$resolved" | grep -q 'not found'; then
    printf '%s\n' "$resolved" >&2
    exit 1
fi
printf '%s\n' "$resolved" | awk '/=> \// {print $3}' | while IFS= read -r library; do
        case "$(basename "$library")" in
            libc.so.*|libm.so.*|libdl.so.*|libpthread.so.*|librt.so.*|libutil.so.*|libanl.so.*|libresolv.so.*|libnss*|libnsl.so.1|ld-linux*|libEGL*|libGLES*|libGL*|libmali*) continue ;;
        esac
        cp -L "$library" "$APP_OUTPUT/lib/$(basename "$library")"
        case "$library" in /opt/*) continue ;; esac
        owner="$(dpkg-query -S "*/$(basename "$library")" 2>/dev/null | head -n1 | sed 's/: \/.*//')"
        if [ -n "$owner" ]; then
            cp "/usr/share/doc/${owner%%:*}/copyright" "$APP_OUTPUT/licenses/${owner%%:*}.txt"
            dpkg-query -W -f='${source:Package}=${source:Version}\n' "$owner" >> "$packages"
        fi
    done

cp /opt/sdl-source/LICENSE.txt "$APP_OUTPUT/licenses/SDL2.txt"
cp /opt/SDL2-source.tar.gz "$APP_OUTPUT/licenses/SDL2-source.tar.gz"
tar -czf "$APP_OUTPUT/licenses/SDL-companions-source.tar.gz" -C /opt companion-source
mkdir -p "$APP_OUTPUT/licenses/runtime-sources"
(cd "$APP_OUTPUT/licenses/runtime-sources" && sort -u "$packages" | xargs -r apt-get source --download-only)
for package in libsdl2-image-2.0-0 libsdl2-ttf-2.0-0 libsdl2-gfx-1.0-0; do
    cp "/usr/share/doc/$package/copyright" "$APP_OUTPUT/licenses/$package.txt"
done
go list -m -f '{{if .Dir}}{{.Path}} {{.Dir}}{{end}}' all | while read -r module dir; do
    [ "$module" = github.com/BrandonKowalski/slot/apps/device-info ] && continue
    name="$(printf '%s' "$module" | tr / _)"
    find "$dir" -maxdepth 1 -type f \( -iname 'LICENSE*' -o -iname 'COPYING*' -o -iname 'NOTICE*' \) \
        | while IFS= read -r license; do
            cp "$license" "$APP_OUTPUT/licenses/$name-$(basename "$license")"
            chmod u+w "$APP_OUTPUT/licenses/$name-$(basename "$license")"
        done
done
cp /src/LICENSE "$APP_OUTPUT/licenses/slot.txt"
cp /src/apps/device-info/HackGen-LICENSE.txt "$APP_OUTPUT/licenses/HackGen-LICENSE.txt"
cp /src/apps/device-info/go.mod "$APP_OUTPUT/licenses/go.mod"
cp /src/apps/device-info/go.sum "$APP_OUTPUT/licenses/go.sum"
# Include this GPL app's corresponding source and vendored Go dependencies.
source_dir="$(mktemp -d)"
cp /src/apps/device-info/*.go /src/apps/device-info/*.sh \
    /src/apps/device-info/go.mod /src/apps/device-info/go.sum \
    /src/apps/device-info/controls.json /src/apps/device-info/Dockerfile \
    /src/apps/device-info/README.md /src/apps/device-info/HackGen-LICENSE.txt "$source_dir/"
cp /src/LICENSE "$source_dir/LICENSE"
(cd "$source_dir" && go mod vendor)
tar -czf "$APP_OUTPUT/licenses/device-info-source.tar.gz" -C "$source_dir" .
rm -rf "$source_dir"
LD_LIBRARY_PATH="$APP_OUTPUT/lib" SLOT_ROOT=/src/sdcard "$APP_OUTPUT/device-info" --text
echo "Device Info ready: $APP_OUTPUT"
