#!/usr/bin/env bash

set -euo pipefail

readonly AUDIO_CPP_COMMIT="3497b7cc44753e2c141d8fe60ac42cec433e3281"
readonly REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly TARGET="${1:-$(rustc -vV | sed -n 's/^host: //p')}"

case "$TARGET" in
  aarch64-apple-darwin) readonly CMAKE_ARCH="arm64" ;;
  x86_64-apple-darwin) readonly CMAKE_ARCH="x86_64" ;;
  *) echo "Unsupported audio.cpp sidecar target: $TARGET" >&2; exit 1 ;;
esac

readonly WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

git clone --filter=blob:none https://github.com/0xShug0/audio.cpp.git "$WORK_DIR/audio.cpp"
git -C "$WORK_DIR/audio.cpp" checkout --detach "$AUDIO_CPP_COMMIT"
git -C "$WORK_DIR/audio.cpp" apply "$REPO_ROOT/scripts/patches/audio-cpp-macos12.patch"

cmake -S "$WORK_DIR/audio.cpp" -B "$WORK_DIR/build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_OSX_ARCHITECTURES="$CMAKE_ARCH" \
  -DCMAKE_OSX_DEPLOYMENT_TARGET=12.0 \
  -DENGINE_ENABLE_METAL=ON \
  -DGGML_METAL_EMBED_LIBRARY=ON \
  -DENGINE_ENABLE_OPENMP=OFF \
  -DENGINE_ENABLE_NATIVE_CPU=OFF \
  -DENGINE_BUILD_EXAMPLES=OFF \
  -DENGINE_BUILD_TESTS=OFF \
  -DENGINE_BUILD_WARMBENCH=OFF \
  -DAUDIOCPP_DEPLOYMENT_BUILD=ON \
  -DAUDIOCPP_BUILD_NATIVE_MODEL_MANAGER=OFF \
  -DAUDIOCPP_MODEL_SET=custom \
  -DAUDIOCPP_MODELS=granite5asr
cmake --build "$WORK_DIR/build" --parallel --target audiocpp_cli

mkdir -p "$REPO_ROOT/src-tauri/binaries" "$REPO_ROOT/src-tauri/resources/licenses"
cp "$WORK_DIR/build/bin/audiocpp_cli" \
  "$REPO_ROOT/src-tauri/binaries/audiocpp_cli-$TARGET"
cp "$WORK_DIR/audio.cpp/LICENSE" \
  "$REPO_ROOT/src-tauri/resources/licenses/audio.cpp-LICENSE"

"$REPO_ROOT/src-tauri/binaries/audiocpp_cli-$TARGET" --list-loaders --json | grep -q granite5asr
