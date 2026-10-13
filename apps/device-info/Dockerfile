FROM golang:1.24-bullseye AS sdk

# Debian's SDL2 has no Mali fbdev backend. Build the SDL fork used by NextUI's
# H700 port, and keep its runtime private to this app.
ARG SDL_COMMIT=d4a7d7503524cc469fe775242f3f925d4dd56c88
RUN printf '%s\n' \
       'deb [check-valid-until=no] https://snapshot.debian.org/archive/debian/20250801T000000Z bullseye main' \
       'deb [check-valid-until=no] https://snapshot.debian.org/archive/debian-security/20250801T000000Z bullseye-security main' \
       > /etc/apt/sources.list \
    && sed -n 's/^deb /deb-src /p' /etc/apt/sources.list > /etc/apt/sources.list.d/source.list \
    && apt-get update && apt-get install -y --no-install-recommends \
       ca-certificates curl build-essential pkg-config autoconf automake libtool patch \
       libsdl2-dev libsdl2-image-dev libsdl2-ttf-dev libsdl2-gfx-dev \
       libegl1-mesa-dev libgles2-mesa-dev libasound2-dev \
       xvfb xauth xdotool imagemagick \
    && mkdir -p /opt/sdl-source /opt/companion-source \
    && curl -fsSL "https://github.com/JohnnyonFlame/SDL-malifbdev-rot/archive/${SDL_COMMIT}.tar.gz" \
       -o /opt/SDL2-source.tar.gz \
    && tar -xzf /opt/SDL2-source.tar.gz --strip-components=1 -C /opt/sdl-source \
    && cd /opt/companion-source \
    && apt-get source --download-only libsdl2-image libsdl2-ttf libsdl2-gfx

# This revision's Autoconf recipe nests Mali under KMSDRM and omits its sources.
# Enable the framebuffer backend independently, keeping DRM/GBM off the device.
COPY sdl-mali-autoconf.patch /opt/sdl-mali-autoconf.patch
RUN cd /opt/sdl-source \
    && patch -p1 < /opt/sdl-mali-autoconf.patch \
    && ./autogen.sh \
    && ./configure --prefix=/opt/slot-sdl --enable-video-mali --enable-video-opengles \
       --disable-video-x11 --disable-video-wayland --disable-video-kmsdrm \
       --disable-video-vulkan --disable-video-opengl --disable-video-rpi \
       --disable-video-directfb --disable-libudev --enable-alsa --enable-alsa-shared \
    && grep -q '^#define SDL_VIDEO_DRIVER_MALI 1$' include/SDL_config.h \
    && make -j4 && make install

WORKDIR /src/apps/device-info
ENV PKG_CONFIG_PATH=/opt/slot-sdl/lib/pkgconfig

# The repository is mounted by build.sh so Go and C caches survive rebuilds.
CMD ["sh", "container-build.sh"]

# On x86, compile ARM64 with a native cross compiler instead of emulating every
# compiler process. The same Bullseye SDL sources and runtime are used either way.
FROM sdk AS cross-device
RUN dpkg --add-architecture arm64 && apt-get update \
    && apt-get install -y --no-install-recommends \
       gcc-aarch64-linux-gnu g++-aarch64-linux-gnu \
       libsdl2-dev:arm64 libsdl2-image-dev:arm64 libsdl2-ttf-dev:arm64 \
       libsdl2-gfx-dev:arm64 libegl1-mesa-dev:arm64 libgles2-mesa-dev:arm64 \
       libasound2-dev:arm64 \
    && cd /opt/sdl-source && make distclean \
    && PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig \
       ./configure --host=aarch64-linux-gnu --build=x86_64-pc-linux-gnu \
       --prefix=/opt/slot-sdl --enable-video-mali --enable-video-opengles \
       --disable-video-x11 --disable-video-wayland --disable-video-kmsdrm \
       --disable-video-vulkan --disable-video-opengl --disable-video-rpi \
       --disable-video-directfb --disable-libudev --enable-alsa --enable-alsa-shared \
    && grep -q '^#define SDL_VIDEO_DRIVER_MALI 1$' include/SDL_config.h \
    && make -j4 && make install
ENV GOARCH=arm64 CGO_ENABLED=1 CC=aarch64-linux-gnu-gcc CXX=aarch64-linux-gnu-g++
ENV PKG_CONFIG_LIBDIR=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig
