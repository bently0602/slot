#!/usr/bin/env bash
# Exercise setup without package installation, network access or a device build.
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
fixture="$(mktemp -d)"
trap 'rm -rf -- "$fixture"' EXIT
mkdir -p "$fixture/repo with spaces" "$fixture/bin" "$fixture/release"
cp "$repo_dir/setup-device.sh" "$fixture/repo with spaces/"
touch "$fixture/repo with spaces/taskfile.yml" "$fixture/repo with spaces/rust-toolchain.toml"
export SETUP_TEST_LOG="$fixture/calls" SETUP_TEST_RELEASE="$fixture/release"
export PATH="$fixture/bin:$PATH"

cat > "$fixture/bin/uname" <<'MOCK'
#!/usr/bin/env bash
case "$1" in -s) echo Linux ;; -m) echo "${SETUP_TEST_HOST_ARCH:-x86_64}" ;; esac
MOCK
cat > "$fixture/bin/dpkg-query" <<'MOCK'
#!/usr/bin/env bash
if [[ ${SETUP_TEST_MISSING_PACKAGE:-} == "${@: -1}" ]]; then exit 1; fi
printf 'install ok installed'
MOCK
cat > "$fixture/bin/apt-get" <<'MOCK'
#!/usr/bin/env bash
echo "apt $*" >> "$SETUP_TEST_LOG"
[[ ${SETUP_TEST_MISSING_PACKAGE:-} != '' ]] || exit 99
MOCK
cat > "$fixture/bin/docker" <<'MOCK'
#!/usr/bin/env bash
echo "docker platform=${DOCKER_DEFAULT_PLATFORM:-unset} $*" >> "$SETUP_TEST_LOG"
case "$1" in
  info) exit "${SETUP_TEST_DOCKER_FAIL:-0}" ;;
  run) echo "${SETUP_TEST_CONTAINER_ARCH:-aarch64}" ;;
esac
MOCK
cat > "$fixture/bin/file" <<'MOCK'
#!/usr/bin/env bash
echo "$1: ELF 64-bit, ${SETUP_TEST_BINARY_ARCH:-aarch64}"
MOCK
cat > "$fixture/bin/task" <<'MOCK'
#!/usr/bin/env bash
case "$1" in
  --help) echo 'Usage: task --taskfile' ;;
  --version) echo 3.54.0 ;;
  dist:device)
    echo "task platform=${DOCKER_DEFAULT_PLATFORM:-unset} $*" >> "$SETUP_TEST_LOG"
    [[ "$2" == OUT=target/device-bundle ]]
    [[ ${SETUP_TEST_BUILD_FAIL:-0} == 0 ]] || exit 18
    for directory in System Games BIOS Config Saves States; do
      mkdir -p "target/device-bundle/$directory"
    done
    for binary in slot mgba_libretro.so gpsp_libretro.so pcsx_rearmed_libretro.so; do
      echo "new software" > "target/device-bundle/System/$binary"
    done
    ;;
  *) exit 2 ;;
esac
MOCK
cat > "$fixture/bin/curl" <<'MOCK'
#!/usr/bin/env bash
echo 'curl' >> "$SETUP_TEST_LOG"
while (( $# )); do
  if [[ $1 == -o ]]; then output=$2; shift; fi
  url=$1
  shift
done
cp "$SETUP_TEST_RELEASE/${url##*/}" "$output"
MOCK
cat > "$fixture/bin/id" <<'MOCK'
#!/usr/bin/env bash
echo 0
MOCK
cat > "$fixture/bin/rustup" <<'MOCK'
#!/usr/bin/env bash
echo "rustup $*" >> "$SETUP_TEST_LOG"
if [[ $1 == run && ${SETUP_TEST_INSTALL_RUST:-0} == 1 ]]; then exit 1; fi
MOCK
chmod +x "$fixture/bin/"*

cd "$fixture/repo with spaces"
for directory in Games BIOS Config Saves States; do
  mkdir -p "dist-device/$directory"
  echo "keep $directory" > "dist-device/$directory/user-content"
done
mkdir -p dist-device/System
echo 'old software' > dist-device/System/slot
DOCKER_DEFAULT_PLATFORM=linux/amd64 bash setup-device.sh > "$fixture/first.log"
bash setup-device.sh > "$fixture/second.log"
for directory in Games BIOS Config Saves States; do
  [[ $(cat "dist-device/$directory/user-content") == "keep $directory" ]]
done
[[ $(cat dist-device/System/slot) == 'new software' ]]
grep -Fq 'task platform=linux/arm64 dist:device OUT=target/device-bundle' "$SETUP_TEST_LOG"
grep -Fq -- '--platform linux/arm64 rust:1-bullseye uname -m' "$SETUP_TEST_LOG"
[[ $(grep -c '^task ' "$SETUP_TEST_LOG") == 2 ]]

expect_failure() {
  if "$@" > "$fixture/failure.log" 2>&1; then
    echo "Expected failure: $*" >&2
    exit 1
  fi
}
bash setup-device.sh --help > "$fixture/help.log"
expect_failure bash setup-device.sh --unknown
expect_failure env SETUP_TEST_DOCKER_FAIL=1 bash setup-device.sh
grep -q 'Docker is not ready' "$fixture/failure.log"
expect_failure env SETUP_TEST_CONTAINER_ARCH=x86_64 bash setup-device.sh
grep -q 'Expected an ARM64 container' "$fixture/failure.log"
echo 'last good bundle' > dist-device/System/slot
expect_failure env SETUP_TEST_BUILD_FAIL=1 bash setup-device.sh
[[ $(cat dist-device/System/slot) == 'last good bundle' ]]
expect_failure env SETUP_TEST_BINARY_ARCH=x86-64 bash setup-device.sh
grep -q 'non-ARM64 binary' "$fixture/failure.log"
[[ $(cat dist-device/System/slot) == 'last good bundle' ]]

# Verify checksum rejection, a local install and reuse on subsequent runs.
cp "$fixture/bin/task" "$fixture/release/task"
tar -czf "$fixture/release/task_linux_amd64.tar.gz" -C "$fixture/release" task
printf '%064d  task_linux_amd64.tar.gz\n' 0 > "$fixture/release/task_checksums.txt"
printf '#!/usr/bin/env bash\necho Taskwarrior\n' > "$fixture/bin/task"
expect_failure bash setup-device.sh
grep -q 'checksum verification' "$fixture/failure.log"
[[ ! -f target/setup-device/bin/task ]]
(cd "$fixture/release" && sha256sum task_linux_amd64.tar.gz > task_checksums.txt)
bash setup-device.sh > "$fixture/install.log"
[[ -x target/setup-device/bin/task ]]
download_count="$(grep -c '^curl$' "$SETUP_TEST_LOG")"
bash setup-device.sh > "$fixture/reuse.log"
[[ $(grep -c '^curl$' "$SETUP_TEST_LOG") == "$download_count" ]]
[[ $(cat dist-device/System/slot) == 'new software' ]]

# Native ARM64 installs the pinned Rust toolchain and missing build packages,
# without touching Docker; restore the valid host Task for this fixture.
cp "$fixture/release/task" "$fixture/bin/task"
printf '[toolchain]\nchannel = "1.96"\n' > rust-toolchain.toml
docker_count="$(grep -c '^docker ' "$SETUP_TEST_LOG")"
env CARGO_HOME="$fixture/empty-cargo" SETUP_TEST_HOST_ARCH=aarch64 \
  SETUP_TEST_MISSING_PACKAGE=cmake SETUP_TEST_INSTALL_RUST=1 \
  bash setup-device.sh > "$fixture/native.log"
[[ $(grep -c '^docker ' "$SETUP_TEST_LOG") == "$docker_count" ]]
grep -Fxq 'apt update' "$SETUP_TEST_LOG"
grep -Fxq 'apt install -y --no-install-recommends cmake' "$SETUP_TEST_LOG"
grep -Fxq 'rustup toolchain install 1.96 --profile minimal --component rustfmt --component clippy' "$SETUP_TEST_LOG"
echo 'setup-device smoke tests passed (reruns, preservation, ARM64 checks, failures, Task checksum/install/cache, native prerequisites).'
