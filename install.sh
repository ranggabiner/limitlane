#!/usr/bin/env bash
set -e

# LimitLane Installation Script
# Usage: curl -fsSL https://raw.githubusercontent.com/binerlabs/limitlane/main/install.sh | bash

REPO="ranggabiner/limitlane"
INSTALL_DIR="${HOME}/.local/bin"

echo "==== Installing LimitLane ===="

# Ensure target directory exists
mkdir -p "${INSTALL_DIR}"

# Check if building from source via cargo is available
if command -v cargo >/dev/null 2>&1; then
    echo "Found cargo! Installing LimitLane via cargo..."
    if [ -f "Cargo.toml" ]; then
        cargo install --path .
    else
        echo "Building/Installing from crates.io..."
        cargo install limitlane || {
            echo "crates.io package not yet published. Clone repository and run 'cargo install --path .'"
            exit 1
        }
    fi
else
    # Fallback to downloading release binary from GitHub Releases
    OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
    ARCH="$(uname -m)"

    case "${ARCH}" in
        x86_64|amd64) ARCH="x86_64" ;;
        aarch64|arm64) ARCH="aarch64" ;;
        *) echo "Unsupported architecture: ${ARCH}"; exit 1 ;;
    esac

    case "${OS}" in
        darwin) OS="apple-darwin" ;;
        linux) OS="unknown-linux-gnu" ;;
        *) echo "Unsupported operating system: ${OS}"; exit 1 ;;
    esac

    TARGET="${ARCH}-${OS}"
    TARBALL="limitlane-${TARGET}.tar.gz"
    RELEASE_URL="https://github.com/${REPO}/releases/latest/download/${TARBALL}"

    echo "Downloading binary release for ${TARGET}..."
    TMP_DIR="$(mktemp -d)"
    trap 'rm -rf "${TMP_DIR}"' EXIT

    if curl -sSLf "${RELEASE_URL}" -o "${TMP_DIR}/${TARBALL}"; then
        tar -xzf "${TMP_DIR}/${TARBALL}" -C "${TMP_DIR}"
        mv "${TMP_DIR}/limitlane" "${INSTALL_DIR}/limitlane"
        chmod +x "${INSTALL_DIR}/limitlane"
        echo "LimitLane installed to ${INSTALL_DIR}/limitlane"
    else
        echo "Could not download pre-built binary. Please install Rust and run: cargo install --path ."
        exit 1
    fi
fi

# PATH guidance
if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]] && [[ ":$PATH:" != *":${HOME}/.cargo/bin:"* ]]; then
    echo ""
    echo "Make sure ${INSTALL_DIR} or ~/.cargo/bin is in your PATH:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
fi

echo "Done! Run 'limitlane' to launch the TUI dashboard."
