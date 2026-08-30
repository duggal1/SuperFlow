#!/bin/bash
# Sapphire Agent Factory (sp) — Full Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/duggal1/sapphire-harness/master/install.sh | bash

set -e

# ── Visual helpers ─────────────────────────────────────────────
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    BOLD=$'\033[1m'
    DIM=$'\033[2m'
    RESET=$'\033[0m'
    FG=$'\033[38;2;248;245;255m'
    MUTED=$'\033[38;2;170;163;191m'
    BORDER=$'\033[38;2;86;79;111m'
    PURPLE=$'\033[38;2;212;128;255m'
    PURPLE_BRIGHT=$'\033[38;2;242;226;255m'
    GREEN=$'\033[38;2;144;244;178m'
    TEAL=$'\033[38;2;133;226;239m'
    YELLOW=$'\033[38;2;244;189;102m'
    RED=$'\033[38;2;255;136;170m'
else
    BOLD=""
    DIM=""
    RESET=""
    FG=""
    MUTED=""
    BORDER=""
    PURPLE=""
    PURPLE_BRIGHT=""
    GREEN=""
    TEAL=""
    YELLOW=""
    RED=""
fi

BOX_WIDTH=68

check() { printf " ${GREEN}✓${RESET} %b\n" "$1"; }
info() { printf " ${PURPLE}›${RESET} ${MUTED}%b${RESET}\n" "$1"; }
warn() { printf " ${YELLOW}!${RESET} %b\n" "$1"; }
err() { printf " ${RED}✕${RESET} %b\n" "$1" >&2; }

repeat_char() {
    local char="$1"
    local count="$2"
    local out=""

    while [ "$count" -gt 0 ]; do
        out="${out}${char}"
        count=$((count - 1))
    done

    printf "%s" "$out"
}

fit_text() {
    local text="$1"
    local width="$2"

    if [ "${#text}" -le "$width" ]; then
        printf "%s" "$text"
    else
        printf "%s" "${text:0:$((width - 3))}..."
    fi
}

init_box_width() {
    local cols="${COLUMNS:-}"
    if [ -z "$cols" ] && command -v tput >/dev/null 2>&1; then
        cols=$(tput cols 2>/dev/null || true)
    fi
    if [ -z "$cols" ]; then
        cols=80
    fi

    if [ "$cols" -lt 54 ]; then
        BOX_WIDTH=46
    else
        BOX_WIDTH=$((cols - 8))
        if [ "$BOX_WIDTH" -gt 76 ]; then
            BOX_WIDTH=76
        fi
    fi
}

print_box_border() {
    printf "${BORDER}┌"
    repeat_char "─" $((BOX_WIDTH + 2))
    printf "┐${RESET}\n"
}

print_box_footer() {
    printf "${BORDER}└"
    repeat_char "─" $((BOX_WIDTH + 2))
    printf "┘${RESET}\n"
}

print_box_line() {
    local text
    text=$(fit_text "$1" "$BOX_WIDTH")
    printf "${BORDER}│ ${FG}%-*s${RESET} ${BORDER}│${RESET}\n" "$BOX_WIDTH" "$text"
}

print_centered_box_line() {
    local text="$1"
    local style_prefix="$2"
    local text_width=${#text}
    local left_pad=0
    local right_pad=0

    if [ "$text_width" -lt "$BOX_WIDTH" ]; then
        left_pad=$(((BOX_WIDTH - text_width) / 2))
        right_pad=$((BOX_WIDTH - text_width - left_pad))
    fi

    printf "${BORDER}│ %*s%b%s%b%*s ${BORDER}│${RESET}\n" \
        "$left_pad" "" \
        "$style_prefix" "$text" "$RESET" \
        "$right_pad" ""
}

print_empty_box_line() {
    printf "${BORDER}│ %-*s │${RESET}\n" "$BOX_WIDTH" ""
}

# ── Config ─────────────────────────────────────────────────────
REPO="sapphire-harness"
GITHUB_USER="duggal1"
BINARY_NAME="sp"
INSTALL_DIR="${HOME}/.local/bin"
VERSION="${SP_VERSION:-latest}"

# ── Banner ────────────────────────────────────────────────────
print_banner() {
    printf "\n"
    print_box_border
    print_empty_box_line
    print_centered_box_line "Sapphire Agent Factory" "${PURPLE_BRIGHT}${BOLD}"
    print_centered_box_line "Terminal-first multi-agent orchestration CLI" "${MUTED}"
    print_empty_box_line
    print_box_footer
    printf "\n"
}

print_capabilities() {
    print_box_border
    printf "${BORDER}│ ${PURPLE_BRIGHT}${BOLD}%-*s${RESET} ${BORDER}│${RESET}\n" "$BOX_WIDTH" "Capabilities"
    print_empty_box_line
    print_box_line "• Plans missions before execution"
    print_box_line "• Runs supervisor and worker terminals in parallel"
    print_box_line "• Preserves mail, replay, status, and mission state locally"
    print_box_line "• Keeps validation, watchdog, and recovery loops active"
    print_box_line "• Supports tmux teamwork surfaces for live coordination"
    print_box_footer
    printf "\n"
}

# ── Detect platform ────────────────────────────────────────────
detect_platform() {
    local os arch
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$os" in
        darwin) os="apple-darwin" ;;
        linux)  os="unknown-linux-gnu" ;;
        *)      err "Unsupported OS: $os"; exit 1 ;;
    esac
    case "$arch" in
        x86_64)       arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *)            err "Unsupported architecture: $arch"; exit 1 ;;
    esac

    printf "\n" >&2
    check "Detected: ${BOLD}${os}${RESET} (${BOLD}${arch}${RESET})" >&2
    printf "%s" "${arch}-${os}"
}

# ── Prerequisites ──────────────────────────────────────────────
check_prereqs() {
    if ! command -v curl &>/dev/null; then
        err "curl is required"; exit 1
    fi
}

# ── Install directory ──────────────────────────────────────────
ensure_install_dir() {
    if [ ! -d "$INSTALL_DIR" ]; then
        info "Creating install directory: ${DIM}${INSTALL_DIR}${RESET}"
        mkdir -p "$INSTALL_DIR"
    fi
}

# ── Build & install ────────────────────────────────────────────
install_binary() {
    local platform="$1"

    if command -v cargo &>/dev/null; then
        local ver
        ver=$(cargo --version 2>/dev/null || echo "unknown")
        check "Rust found: ${DIM}${ver}${RESET}"
    else
        err "Rust/Cargo is required for source installation"
        printf "  Install with: ${BOLD}curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh${RESET}\n\n"
        exit 1
    fi

    info "Building release binary..."
    printf "  ${DIM}(this may take a minute)${RESET}\n\n"

    local build_dir
    if [ -f "Cargo.toml" ] && grep -q "sapphire-agent-factory" Cargo.toml 2>/dev/null; then
        info "Using current directory as source"
        build_dir="$(pwd)"
    else
        build_dir=$(mktemp -d)
        info "Cloning repository..."
        git clone --depth 1 "https://github.com/${GITHUB_USER}/${REPO}.git" "$build_dir" 2>/dev/null || {
            err "Failed to clone repository"
            exit 1
        }
    fi

    cd "$build_dir"
    cargo build --release --quiet

    if [ ! -f "target/release/${BINARY_NAME}" ]; then
        err "Build failed: binary not found"
        exit 1
    fi

    info "Installing to ${DIM}${INSTALL_DIR}/${BINARY_NAME}${RESET}"
    cp "target/release/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    if [ "$build_dir" != "$(pwd)" ]; then
        rm -rf "$build_dir"
    fi
}

# ── Verify ─────────────────────────────────────────────────────
verify_installation() {
    local binary_path="${INSTALL_DIR}/${BINARY_NAME}"

    if [ ! -f "$binary_path" ]; then
        err "Installation failed: binary not found at ${DIM}${binary_path}${RESET}"
        exit 1
    fi

    if [ ! -x "$binary_path" ]; then
        err "Installation failed: binary is not executable"
        exit 1
    fi

    printf "\n"
    check "Installed successfully: ${BOLD}${binary_path}${RESET}"
}

# ── PATH check ─────────────────────────────────────────────────
check_path() {
    if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
        printf "\n"
        warn "${DIM}${INSTALL_DIR}${RESET} is not in your PATH"
        printf "  ${DIM}echo 'export PATH=\"${INSTALL_DIR}:\\\$PATH\"' >> ~/.zshrc${RESET}\n"
        printf "  ${DIM}source ~/.zshrc${RESET}\n"
    fi
}

# ── Quick start ────────────────────────────────────────────────
print_quickstart() {
    printf "\n"
    print_box_border
    printf "${BORDER}│ ${PURPLE_BRIGHT}${BOLD}%-*s${RESET} ${BORDER}│${RESET}\n" "$BOX_WIDTH" "Quick start"
    print_empty_box_line
    print_box_line "sp claude 2 \"debug and validate the repo\""
    print_box_line "sp ns claude 3 \"prompt 1\" \"prompt 2\" \"prompt 3\""
    print_box_line "sp status"
    print_box_line "sp --help"
    print_empty_box_line
    print_box_line "Docs: https://github.com/${GITHUB_USER}/${REPO}"
    print_box_footer
    printf "\n"
}

# ── Main ───────────────────────────────────────────────────────
main() {
    init_box_width
    print_banner
    print_capabilities
    check_prereqs
    local platform
    platform=$(detect_platform)
    ensure_install_dir
    install_binary "$platform"
    verify_installation
    check_path
    print_quickstart

    check "Ready to orchestrate ✨"
    printf "\n"
}

main "$@"
