#!/usr/bin/env bash

set -Eeuo pipefail

PROJECT_DIR="${MLX_PROJECT_DIR:-$HOME/mlx-voice}"
PYTHON_VERSION="${PYTHON_VERSION:-3.12}"

MLX_VERSION="0.32.1"
MLX_LM_VERSION="0.31.3"
MLX_AUDIO_VERSION="0.5.0"
PARAKEET_MLX_VERSION="0.5.2"

die() {
    printf "\nERROR: %s\n" "$*" >&2
    exit 1
}

info() {
    printf "\n==> %s\n" "$*"
}

# ---------------------------------------------------------------------------
# Platform validation
# ---------------------------------------------------------------------------

[[ "$(uname -s)" == "Darwin" ]] || \
    die "This setup is for macOS."

[[ "$(uname -m)" == "arm64" ]] || \
    die "You must run this natively on Apple Silicon, not under Rosetta."

MACOS_VERSION="$(sw_vers -productVersion)"
MACOS_MAJOR="${MACOS_VERSION%%.*}"

if (( MACOS_MAJOR < 14 )); then
    die "MLX requires macOS 14 or newer."
fi

echo
echo "MLX Native Apple Silicon Setup"
echo "------------------------------"
echo "macOS:        $MACOS_VERSION"
echo "Architecture: $(uname -m)"
echo "Project:      $PROJECT_DIR"

# ---------------------------------------------------------------------------
# uv
# ---------------------------------------------------------------------------

if ! command -v uv >/dev/null 2>&1; then
    info "Installing uv standalone"

    curl -LsSf https://astral.sh/uv/install.sh | sh

    export PATH="$HOME/.local/bin:$PATH"
fi

command -v uv >/dev/null 2>&1 || \
    die "uv installation failed."

echo "uv:           $(uv --version)"

# ---------------------------------------------------------------------------
# Python
# ---------------------------------------------------------------------------

info "Installing managed Python $PYTHON_VERSION"

uv python install "$PYTHON_VERSION"

mkdir -p "$PROJECT_DIR"
cd "$PROJECT_DIR"

# ---------------------------------------------------------------------------
# Main environment
# ---------------------------------------------------------------------------

info "Creating main MLX environment"

rm -rf .venv

uv venv \
    --python "$PYTHON_VERSION" \
    .venv

PYTHON="$PROJECT_DIR/.venv/bin/python"

# ---------------------------------------------------------------------------
# Native binary-only MLX stack
#
# --only-binary=:all:
#
# This is intentional. If a dependency unexpectedly wants a C/C++ source
# build, installation fails rather than quietly demanding compiler tooling.
# ---------------------------------------------------------------------------

info "Installing MLX + MLX-LM + MLX-Audio"

uv pip install \
    --python "$PYTHON" \
    --only-binary=:all: \
    "mlx==$MLX_VERSION" \
    "mlx-lm==$MLX_LM_VERSION" \
    "mlx-audio[stt,llm]==$MLX_AUDIO_VERSION" \
    "parakeet-mlx==$PARAKEET_MLX_VERSION"

# Hugging Face high-performance model downloads.
uv pip install \
    --python "$PYTHON" \
    --only-binary=:all: \
    "huggingface-hub[hf-xet]>=1.0" \
    "safetensors>=0.6"

# ---------------------------------------------------------------------------
# Verify installation
# ---------------------------------------------------------------------------

info "Testing MLX + Metal"

"$PYTHON" - <<'PY'
import platform
import sys

import mlx.core as mx

print()
print("Python:", sys.version.split()[0])
print("Executable:", sys.executable)
print("Architecture:", platform.machine())
print("Metal:", mx.metal.is_available())

if platform.machine() != "arm64":
    raise SystemExit("FAIL: Python is not native arm64.")

if not mx.metal.is_available():
    raise SystemExit("FAIL: Metal backend unavailable.")

# Actually execute something substantial enough to touch the backend.
a = mx.random.normal((1024, 1024))
b = mx.random.normal((1024, 1024))
c = a @ b

mx.eval(c)

print("Tensor:", c.shape)
print("MLX + Metal: PASS")
PY

cat <<EOF

==============================================================
MLX ENVIRONMENT READY
==============================================================

Project:
    $PROJECT_DIR

Activate:
    cd "$PROJECT_DIR"
    source .venv/bin/activate

Python:
    $PYTHON

Full Xcode app:
    NOT REQUIRED

EOF

# ---------------------------------------------------------------------------
# Stage runtime scripts into $PROJECT_DIR so the app can find them without
# the repo checkout (Rust resolves $HOME/mlx-voice/mlx_voice.py first).
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

info "Staging mlx_voice.py + doctor.py into $PROJECT_DIR"
cp "$SCRIPT_DIR/mlx_voice.py" "$PROJECT_DIR/mlx_voice.py"
cp "$SCRIPT_DIR/doctor.py" "$PROJECT_DIR/doctor.py"

# ---------------------------------------------------------------------------
# Optional: pull the tiny validation model so Metal inference can be proven
# right after setup. Set MLX_SKIP_TEST_MODEL=1 to skip this download.
# ---------------------------------------------------------------------------

if [[ -n "${HUGGING_FACE_TOKEN:-}" ]]; then
    export HF_TOKEN="${HUGGING_FACE_TOKEN}"
fi

if [[ "${MLX_SKIP_TEST_MODEL:-0}" != "1" ]]; then
    info "Fetching validation model animaslabs/parakeet-tdt-0.6b-v3-mlx-4bit (~600 MB)"
    "$PYTHON" -c "from huggingface_hub import snapshot_download; snapshot_download('animaslabs/parakeet-tdt-0.6b-v3-mlx-4bit')" \
        || info "Validation model download failed (network?) — transcribe will fetch it on first use."
fi

echo
info "Done. Transcripts run through $PROJECT_DIR/mlx_voice.py."
