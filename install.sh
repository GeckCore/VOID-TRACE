#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════
#  AI-GHOST-HUNTER — install.sh
#  Installs Rust (if needed), builds the binary, and links it
#  to /usr/local/bin/aigh so it works anywhere.
# ═══════════════════════════════════════════════════════════
set -euo pipefail

RED='\033[1;31m'; YEL='\033[1;33m'; GRN='\033[1;32m'
CYN='\033[1;36m'; WHT='\033[1;37m'; RST='\033[0m'

banner() {
  echo -e "${RED}"
  echo "  ╔══════════════════════════════════════════╗"
  echo "  ║   AI-GHOST-HUNTER  ·  Build & Install    ║"
  echo "  ╚══════════════════════════════════════════╝"
  echo -e "${RST}"
}

step()  { echo -e "${CYN}►${RST} ${WHT}$*${RST}"; }
ok()    { echo -e "${GRN}✓${RST} $*"; }
warn()  { echo -e "${YEL}⚠${RST}  $*"; }
die()   { echo -e "${RED}✗ FATAL:${RST} $*" >&2; exit 1; }

# ── 0. Detect OS ─────────────────────────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"
step "Detected: ${OS} / ${ARCH}"

# ── 1. Check / Install system deps ───────────────────────────────────────────
need_pkg() {
  command -v "$1" &>/dev/null
}

install_dep() {
  local pkg="$1"
  step "Installing system dependency: ${pkg}"
  if command -v apt-get &>/dev/null; then
    sudo apt-get install -y --no-install-recommends "$pkg"
  elif command -v brew &>/dev/null; then
    brew install "$pkg"
  elif command -v pacman &>/dev/null; then
    sudo pacman -S --noconfirm "$pkg"
  elif command -v dnf &>/dev/null; then
    sudo dnf install -y "$pkg"
  else
    die "Cannot auto-install ${pkg}. Install it manually and re-run."
  fi
}

need_pkg git  || install_dep git
need_pkg curl || install_dep curl
# C compiler — required by tree-sitter grammar build scripts
need_pkg cc   || {
  if command -v apt-get &>/dev/null; then
    step "Installing build-essential (C compiler for tree-sitter)"
    sudo apt-get install -y --no-install-recommends build-essential
  elif command -v brew &>/dev/null; then
    warn "Xcode Command Line Tools may be needed: xcode-select --install"
  else
    die "No C compiler found. Install gcc or clang."
  fi
}

# ── 2. Install Rust if not present ───────────────────────────────────────────
if ! command -v cargo &>/dev/null; then
  step "Rust not found — installing via rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --no-modify-path

  # Source the env for the rest of this script
  # shellcheck source=/dev/null
  source "${HOME}/.cargo/env"
  ok "Rust installed: $(rustc --version)"
else
  ok "Rust already present: $(rustc --version)"
  # Make sure cargo is on PATH (some shells miss it)
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi

# Ensure at least 1.70
RUST_MINOR=$(rustc --version | grep -oP '1\.\K\d+')
if [[ "${RUST_MINOR}" -lt 70 ]]; then
  step "Updating Rust (need ≥ 1.70, have 1.${RUST_MINOR})"
  rustup update stable
fi

# ── 3. Locate project root (script must live next to Cargo.toml) ─────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ ! -f "${SCRIPT_DIR}/Cargo.toml" ]]; then
  die "Cargo.toml not found next to install.sh (expected at ${SCRIPT_DIR})"
fi

# ── 4. Build ──────────────────────────────────────────────────────────────────
step "Building AI-Ghost-Hunter (release profile) — this takes 2-5 min on first run"
cd "${SCRIPT_DIR}"
cargo build --release 2>&1

BINARY="${SCRIPT_DIR}/target/release/aigh"
[[ -f "${BINARY}" ]] || die "Build succeeded but binary not found at ${BINARY}"
ok "Binary built: ${BINARY}  ($(du -sh "${BINARY}" | cut -f1))"

# ── 5. Install to PATH ────────────────────────────────────────────────────────
INSTALL_DIR="/usr/local/bin"

# Fallback to ~/.local/bin if we can't write to /usr/local/bin
if [[ ! -w "${INSTALL_DIR}" ]]; then
  INSTALL_DIR="${HOME}/.local/bin"
  mkdir -p "${INSTALL_DIR}"
  warn "/usr/local/bin not writable — installing to ${INSTALL_DIR}"
  warn "Add this to your shell profile if not already present:"
  warn "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
fi

cp "${BINARY}" "${INSTALL_DIR}/aigh"
chmod 755 "${INSTALL_DIR}/aigh"
ok "Installed to: ${INSTALL_DIR}/aigh"

# ── 6. Shell completion (optional) ───────────────────────────────────────────
SHELL_NAME="$(basename "${SHELL:-bash}")"
COMPLETION_INSTALLED=false

case "${SHELL_NAME}" in
  bash)
    COMP_DIR="${HOME}/.local/share/bash-completion/completions"
    mkdir -p "${COMP_DIR}"
    "${INSTALL_DIR}/aigh" completions bash > "${COMP_DIR}/aigh" 2>/dev/null \
      && COMPLETION_INSTALLED=true || true
    ;;
  zsh)
    COMP_DIR="${HOME}/.zsh/completions"
    mkdir -p "${COMP_DIR}"
    "${INSTALL_DIR}/aigh" completions zsh > "${COMP_DIR}/_aigh" 2>/dev/null \
      && COMPLETION_INSTALLED=true || true
    ;;
  fish)
    COMP_DIR="${HOME}/.config/fish/completions"
    mkdir -p "${COMP_DIR}"
    "${INSTALL_DIR}/aigh" completions fish > "${COMP_DIR}/aigh.fish" 2>/dev/null \
      && COMPLETION_INSTALLED=true || true
    ;;
esac

${COMPLETION_INSTALLED} && ok "Shell completions installed for ${SHELL_NAME}"

# ── 7. Smoke test ────────────────────────────────────────────────────────────
step "Smoke test"
"${INSTALL_DIR}/aigh" --version && ok "Binary runs correctly"

# ── 8. Done ───────────────────────────────────────────────────────────────────
echo ""
echo -e "${GRN}══════════════════════════════════════════════════${RST}"
echo -e "${WHT}  Installation complete!${RST}"
echo ""
echo -e "  ${CYN}Quick start:${RST}"
echo -e "  ${WHT}aigh https://github.com/owner/repo${RST}"
echo -e "  ${WHT}aigh ./local/project --verbose${RST}"
echo -e "  ${WHT}aigh ./local/project --json | jq '.global_ai_score'${RST}"
echo ""
echo -e "  ${CYN}Private repos:${RST}"
echo -e "  ${WHT}export GITHUB_TOKEN=ghp_xxxxxxxxxxxx${RST}"
echo -e "  ${WHT}aigh https://github.com/org/private-repo${RST}"
echo -e "${GRN}══════════════════════════════════════════════════${RST}"
echo ""

# Reload hint if PATH was changed
if [[ "${INSTALL_DIR}" == *".local/bin"* ]]; then
  echo -e "${YEL}⚠  Open a new terminal (or run: source ~/.bashrc) for 'aigh' to be on PATH${RST}"
fi
