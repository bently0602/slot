# Setup, building, and installing slot

This guide covers building the Linux ARM64 handheld version of slot from a
Windows computer using Ubuntu in WSL2. It also covers copying the resulting
package to an SD card and rebuilding after you change your fork.

**Run all Bash commands in your Ubuntu terminal.** Blocks marked PowerShell run
in Windows instead. Run build commands from the repository root, which contains
`taskfile.yml` and `Cargo.toml`.

## Quick start: tools are already installed

Start Docker Desktop if that is your Docker backend, then open Ubuntu:

```bash
cd /mnt/c/Users/Administrator/Repos/slot

docker info
docker run --rm --platform linux/arm64 busybox uname -m

export DOCKER_DEFAULT_PLATFORM=linux/arm64
task dist:device OUT=dist-device

file dist-device/System/slot \
     dist-device/System/mgba_libretro.so \
     dist-device/System/gpsp_libretro.so
```

The container test should print `aarch64`, and all three packaged binaries should
be reported as ARM aarch64. The finished package is in `dist-device/`.

Copy **the contents of `dist-device/`** to your slot SD card, so that `System/`
is directly at the card root. See [Installing on a handheld](#installing-on-a-handheld)
for a fresh installation.

> `task dist:device` deletes and recreates its output directory. Keep your personal
> games, saves, and settings on the SD card or in a separate directory. Never set
> `OUT` to your SD card, repository root, or a folder containing personal files.

## 1. Install WSL2 and Ubuntu

Skip this step if Ubuntu already runs in WSL2.

Open **PowerShell as Administrator**:

```powershell
wsl --install -d Ubuntu
```

Restart Windows if prompted, open Ubuntu, and create your Linux username and
password. Then check the installation from PowerShell:

```powershell
wsl --update
wsl --list --verbose
```

Ubuntu should show `VERSION 2`. If it shows version 1:

```powershell
wsl --set-version Ubuntu 2
```

Use the actual distribution name shown by `wsl --list --verbose` if yours is
named something other than `Ubuntu`.

Reference: [Microsoft WSL installation guide](https://learn.microsoft.com/en-us/windows/wsl/install).

## 2. Install basic tools inside Ubuntu

```bash
sudo apt update
sudo apt install -y build-essential git curl ca-certificates unzip file
```

`file` is required because the device tasks check the architecture of the output.
The handheld build runs its Rust compiler and emulator-core build tools inside
Docker; the mGBA build script installs CMake there when necessary.

## 3. Install Rust inside Ubuntu

Install Rust in **WSL**, where you will run the development commands:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup toolchain install 1.96 --component rustfmt --component clippy

rustc --version
cargo --version
```

The repository's [rust-toolchain.toml](rust-toolchain.toml) selects Rust `1.96`
and includes Rustfmt and Clippy. Windows Rust is optional for separate native
Windows development; it is not needed for this WSL workflow.

Reference: [Rust installation instructions](https://rust-lang.org/tools/install/).

## 4. Install Go Task inside Ubuntu

The command is `task`, provided by [Go Task](https://taskfile.dev/docs/installation).
You do not need to install the Go language to use the packaged binary.

```bash
curl -1sLf 'https://dl.cloudsmith.io/public/task/task/setup.deb.sh' | sudo -E bash
sudo apt update
sudo apt install -y task
task --version
```

The project uses Task v3 and defines its commands in [taskfile.yml](taskfile.yml).
Later, from the repository root, `task --list` shows the available tasks.

## 5. Make Docker usable inside WSL

Choose **one** Docker backend for Ubuntu: Docker Desktop's WSL integration, or
Docker Engine installed inside Ubuntu.

### Option A: Docker Desktop

For supported Windows 10/11 installations:

1. Install and start [Docker Desktop](https://docs.docker.com/desktop/setup/install/windows-install/).
2. Use **Linux containers**. If Docker's tray menu offers **Switch to Linux
   containers**, select it.
3. Under **Settings → General**, enable **Use the WSL 2 based engine**.
4. Under **Settings → Resources → WSL Integration**, enable your Ubuntu distribution.
5. Apply the changes and open a new Ubuntu terminal.

In Ubuntu:

```bash
hash -r
docker version
docker info
docker run --rm hello-world
```

`docker version` should show both a client and a server. Docker Desktop must stay
running while you build.

Docker Desktop is not officially supported on Windows Server. For a new Server
setup, use Option B. If your existing Desktop integration already works, you can
continue to the ARM64 check without installing another engine.

Reference: [Docker Desktop WSL integration](https://docs.docker.com/desktop/features/wsl/).

### Option B: Docker Engine inside Ubuntu

Use this when you are building without Docker Desktop, including a Windows Server
development setup. Disable Desktop integration for this distribution if you are
switching to this backend.

The following uses Docker's convenience installer, which Docker documents for
development and testing environments:

```bash
cd ~
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
sudo systemctl enable --now docker
sudo usermod -aG docker "$USER"
newgrp docker

hash -r
docker context use default
docker run --rm hello-world
```

Membership in the `docker` group gives privileged access to the Docker engine.
`newgrp docker` starts a shell with your updated group membership; opening a fresh
WSL session also works.

These service commands require systemd in Ubuntu. If `systemctl` reports that
systemd is unavailable, enable it using
[Microsoft's WSL systemd instructions](https://learn.microsoft.com/en-us/windows/wsl/systemd),
save your work, and restart WSL before retrying.

Reference: [Docker Engine on Ubuntu](https://docs.docker.com/engine/install/ubuntu/)
and [Docker group setup](https://docs.docker.com/engine/install/linux-postinstall/).

### Verify ARM64 containers

The handheld binaries must be Linux ARM64, even when your PC is x86-64:

```bash
docker run --rm --platform linux/arm64 busybox uname -m
```

Expected output:

```text
aarch64
```

Docker Desktop includes support for emulated ARM64 containers. A standalone
Docker Engine may need QEMU/binfmt registration. If the test fails with
`exec format error`, run:

```bash
docker run --privileged --rm --platform linux/amd64 tonistiigi/binfmt --install arm64
docker run --rm --platform linux/arm64 busybox uname -m
```

The installer command above assumes an x86-64 PC. If emulation stops working after
a WSL restart, repeat the check and registration as needed.

Reference: [Docker's manual QEMU setup](https://docs.docker.com/build/building/multi-platform/#install-qemu-manually).

## 6. Open your repository

For the existing Windows checkout on this machine:

```bash
cd /mnt/c/Users/Administrator/Repos/slot
task --list
```

You can also keep a checkout in Ubuntu's filesystem, which generally performs
better for Linux builds than `/mnt/c`. To create a new checkout of your own fork,
replace `YOUR_USERNAME` below:

```bash
mkdir -p ~/Repos
git clone https://github.com/YOUR_USERNAME/slot.git ~/Repos/slot
cd ~/Repos/slot
```

Use one checkout consistently so you build the files you actually edited. For
an Ubuntu checkout, you can open its directory from Windows Explorer through
`\\wsl.localhost\Ubuntu\home\YOUR_LINUX_USERNAME\Repos\slot`.

## 7. Build the complete handheld package

From the repository root in Ubuntu:

```bash
export DOCKER_DEFAULT_PLATFORM=linux/arm64
task dist:device OUT=dist-device
```

**Set that environment variable again in each new terminal.** The current
`device:sh` task uses `rust:1-bullseye` without an explicit `--platform` argument,
so this variable selects the ARM64 image on an x86-64 computer.

For a single command without changing the rest of the terminal session:

```bash
DOCKER_DEFAULT_PLATFORM=linux/arm64 task dist:device OUT=dist-device
```

The task builds the frontend, builds both patched emulator cores, creates the
content folders, and packages scripts, licenses, and core source metadata.
It downloads the required container image and sources automatically.

The first build can take tens of minutes under ARM64 emulation. Later builds can
reuse output in `target-device/` and `vendor/`, although temporary containers may
still download tools and dependencies. Avoid `task clean` for routine retries.

| Command | Result |
| --- | --- |
| `task build:device` | Frontend only: `target-device/device/slot` |
| `task core:device` | mGBA core: `vendor/mgba_libretro.so` |
| `task core:gpsp` | gpSP core: `vendor/gpsp_libretro.so`, plus source archive and metadata |
| `task dist:device OUT=dist-device` | Complete SD-card tree: `dist-device/` |

You only need the last command for a complete package. The individual commands
help isolate a failing component. They need the same ARM64 environment setting.

## 8. Verify the result and run checks

```bash
file dist-device/System/slot \
     dist-device/System/mgba_libretro.so \
     dist-device/System/gpsp_libretro.so

task fmt:check
task test:card
```

All three binary descriptions should contain `aarch64`. Formatting checks inspect
the Rust source; `test:card` runs the card-script tests without a handheld.

This workflow was verified with the complete device package and card-script tests
on this WSL checkout. A successful build verifies compilation and packaging;
test your changes on the handheld to verify graphics, audio, controls, and gameplay.

### Optional desktop build in WSL

For the default host build, install the additional Linux development libraries:

```bash
sudo apt install -y pkg-config libasound2-dev libudev-dev
task build
```

This builds the default desktop frontend in `target/debug/`, rather than the
handheld package. The audio and controller requirements are documented by
[CPAL](https://github.com/RustAudio/cpal#linux-build-dependencies) and
[gilrs](https://docs.rs/gilrs/0.11.0/gilrs/#linuxbsd-evdev).

The current `task core`, `task run`, and `task dist` host recipes assume macOS
cores (`.dylib` and Mach-O). Some host tests/examples also use a macOS-only
`HeadlessSurface`. Consequently, `task test`, `task lint`, and `task check` are
not reliable WSL validation commands without further portability changes.
Use `dist:device` for the handheld workflow.

## Installing on a handheld

The [README](README.md#supported-devices) lists supported models. The following
is the two-card installation described by the project's
[user guide](https://slot-cfw.fyi); see that guide for single-card installation.

1. Download the appropriate [BaseOS image](https://github.com/pvaibhav/BaseOS/releases/latest)
   and flash it to the **SD1** card. Flashing replaces the card's contents.
2. Prepare the **SD2** card as FAT32 or exFAT with an MBR partition scheme.
   Formatting replaces the card's contents too.
3. Copy everything **inside** `dist-device/` to the SD2 root. Do not copy it into
   an extra `dist-device` directory on the card.
4. Put your unzipped games in `Games/GB/`, `Games/GBC/`, or `Games/GBA/`.
5. Safely eject the cards, insert them into the handheld, and boot.

The card should look like this:

```text
SD2 root/
├── BIOS/
├── Config/
├── Games/
│   ├── GB/
│   ├── GBC/
│   └── GBA/
├── Labels/
├── Saves/
├── States/
├── System/
│   ├── slot
│   ├── mgba_libretro.so
│   ├── gpsp_libretro.so
│   ├── licenses/
│   └── ...scripts...
└── Wallpapers/
```

ROMs must be uncompressed `.gb`, `.gbc`, or `.gba` files; `.zip` and `.7z` are
unsupported. ROMs and BIOS files are not required to compile the project.
Optional BIOS files go in `BIOS/`; the user guide lists `gb_bios.bin`,
`gbc_bios.bin`, and `gba_bios.bin`.

### Updating an existing slot installation

Back up your card, then copy the new package's **`System/` contents** over the
existing `System/` directory. Keep `Games/`, `Saves/`, `States/`, and `Config/`.

When updating from a version before 1.4.0, preserve the existing `System/`
directory and overwrite its files instead of deleting it first. The frontend
migrates old settings from `System/` to `Config/` on first boot.

### Optional deployment over ADB

If the handheld exposes ADB to Ubuntu, you can deploy without removing the card:

```bash
sudo apt install -y adb
adb devices
export DOCKER_DEFAULT_PLATFORM=linux/arm64
task deploy:device SD=/mnt/sdcard
```

This task builds and copies the frontend, cores, scripts, and license files to
an existing installation, then **reboots the handheld**. Finish your game session
first. `SD` must match the card's mount path on the device.

Read the device log with:

```bash
task log:device SD=/mnt/sdcard
```

## Troubleshooting

### Permission denied at `/var/run/docker.sock`

Inspect your effective groups and the socket:

```bash
id
ls -l /var/run/docker.sock
getent group docker
```

If the socket belongs to the `docker` group, add your user if needed and refresh
the shell's group membership:

```bash
sudo usermod -aG docker "$USER"
newgrp docker
docker run --rm hello-world
```

If you were already listed in the group, `newgrp docker` or a fresh WSL session
is usually the missing step. Set `DOCKER_DEFAULT_PLATFORM=linux/arm64` again
before retrying the build. Do not work around this by running the entire build
as root or making the socket world-writable.

### Docker command unavailable, or Linux engine unreachable

For Docker Desktop, start the app and enable Ubuntu under WSL Integration.
Open a fresh Ubuntu terminal and run `hash -r`, then `docker info`.

For a standalone Ubuntu engine, run `sudo systemctl start docker`, then
`docker info`. If you previously configured a remote engine, inspect
`docker context ls` and `env | grep '^DOCKER_'`; a stale `DOCKER_HOST` or
`DOCKER_CONTEXT` may be directing the client to the wrong engine.

### Build finishes compiling but fails an `aarch64` check

The container may have built an x86-64 binary. Check the result and retry with
the platform set:

```bash
file target-device/device/slot
docker run --rm --platform linux/arm64 busybox uname -m
export DOCKER_DEFAULT_PLATFORM=linux/arm64
task dist:device OUT=dist-device
```

If the ARM64 container test reports `exec format error`, follow the emulation
registration instructions in step 5.

### `set: Illegal option` or errors containing `\r`

This usually means a shell script has Windows CRLF line endings. The repository's
[.gitattributes](.gitattributes) sets `*.sh text eol=lf`. For an existing checkout
with CRLF scripts, normalize tracked shell scripts from Ubuntu:

```bash
git ls-files -z '*.sh' | xargs -0 -r sed -i 's/\r$//'
```

This preserves edits while removing carriage returns at line ends. Keep shell
scripts saved with LF in your editor. Core patch files have a separate `-text`
rule and must not be normalized.

### Git reports `detected dubious ownership` while building mGBA

Docker runs as root while the mounted checkout can belong to your WSL user.
The current [mGBA build script](cores/mgba/build.sh) handles this by adding only
the generated mGBA source directory to the container's Git `safe.directory`
configuration when required. Use this updated script if your fork has an older
copy; avoid trusting every directory with `safe.directory '*'`.

### Only the final Task failure message is visible

The final `exit status` line identifies the failed task, but the useful compiler
or shell error appears earlier. Run the failing component on its own and save
the complete output, for example:

```bash
export DOCKER_DEFAULT_PLATFORM=linux/arm64
set -o pipefail
task core:device 2>&1 | tee /tmp/slot-mgba-build.log
```

Replace `core:device` with `core:gpsp` or `build:device` as appropriate. After
fixing the cause, rerun `task dist:device OUT=dist-device` to assemble the package.

## Everyday development loop

1. Edit your fork's source files.
2. Run `task fmt:check` and the checks relevant to your change. Use `task fmt`
   when you want Rustfmt to update formatting.
3. Run `task test:card` after changing card scripts.
4. Run `DOCKER_DEFAULT_PLATFORM=linux/arm64 task dist:device OUT=dist-device`.
5. Update the handheld's `System/` files and test the changed behavior on-device.

Keep `target-device/` and `vendor/` between builds to retain reusable output.
