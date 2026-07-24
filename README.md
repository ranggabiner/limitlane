# LimitLane

> **Keep your agents moving.**  
> Monitor every coding-account limit from one terminal.

[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)]()

LimitLane is a native, local-first terminal dashboard for monitoring usage limits across multiple AI coding provider accounts (**Anthropic Claude Code** and **OpenAI Codex**) in real time.

---

## Features

- 🎯 **Unified Terminal Dashboard:** Compare all registered provider accounts in a single Ratatui TUI or query via CLI.
- ⚡ **Limiting Window Evaluator:** Automatically identifies the specific hard limit closest to exhaustion (e.g., 5-hour session, weekly Sonnet, or shared agentic pool).
- 💳 **Paid Overflow & Credit Tracking:** Distinguishes included subscription usage from additional purchased credits and paid-overflow state.
- 🔒 **Secure by Default:** Credentials stored securely in OS native keyrings (macOS Keychain, Linux Secret Service, Windows Credential Manager). Zero telemetry, local storage only.
- 🩺 **Built-in Diagnostics:** `limitlane doctor` verifies system compatibility, keyring access, SQLite health, network connectivity, and provider CLI installations.
- 📊 **Automation & JSON Support:** Export versioned JSON (`--json`) for scripting, shell prompts, and continuous agent loops.
- 🖥️ **Responsive TUI:** Native layout that dynamically scales down to 80×24 terminals.

---

## Supported Providers & Plans

| Provider | Supported Plans | Monitored Windows & Scopes |
|---|---|---|
| **Anthropic Claude** | Pro, Max 5x, Max 20x, Team, Enterprise | 5-hour sessions, weekly all-models, weekly Sonnet, model/feature caps, extra usage credits |
| **OpenAI Codex** | Plus, Pro, Business, Enterprise, Edu | Provider-defined dynamic windows, shared agentic pools, token credit consumption, spend limits |

---

## Installation

### Prerequisites

- **Rust** (1.75+ recommended)
- OS native keyring support (`libsecret` on Linux)

### One-Line Command Install (Recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/ranggabiner/limitlane/main/install.sh | bash
```

### Fast Local Command Install (Cargo)

Run in repository root:

```bash
cargo install --path .
```

Binary `limitlane` langsung ter-install ke PATH (`~/.cargo/bin/limitlane`). Bisa dipanggil langsung dari terminal mana saja:

```bash
limitlane
```

### Building from Source

```bash
git clone https://github.com/binerlabs/limitlane.git
cd limitlane
cargo build --release
```

Binary hasil build ada di `./target/release/limitlane`.

---

## Quick Start

### 1. Launch TUI Dashboard

Simply run `limitlane` without arguments to open the interactive dashboard:

```bash
limitlane
```

#### TUI Keyboard Controls

| Key | Action |
|---|---|
| `j` / `↓` | Select next account |
| `k` / `↑` | Select previous account |
| `Enter` | Open account details |
| `Esc` | Return to main dashboard |
| `Tab` | Cycle screens (Dashboard, Accounts, Credits, Filters) |
| `r` | Refresh selected account |
| `R` | Refresh all accounts |
| `a` | Open accounts management |
| `d` | Open dashboard |
| `f` | Open filters |
| `/` | Search / filter |
| `?` | Toggle help overlay |
| `q` | Quit |

---

### 2. CLI Commands

LimitLane can also be queried directly from your shell or scripts:

```bash
# Check status summary
limitlane status

# Output versioned JSON for scripting
limitlane status --json

# View detailed usage breakdown for an account
limitlane usage --account acc_123

# Filter usage by provider
limitlane usage --provider claude

# Manage accounts
limitlane accounts list
limitlane accounts add codex
limitlane accounts add claude
limitlane accounts remove <account-id>

# Run manual usage refresh
limitlane refresh

# Run system health diagnostics
limitlane doctor --json
```

---

## Architecture

LimitLane is written entirely in **Rust** with no webview, Tauri, React, Node.js, or Bun dependencies:

- **TUI Layer:** Built with [Ratatui](https://github.com/ratatui-org/ratatui) and [Crossterm](https://github.com/crossterm-rs/crossterm).
- **Async Runtime:** Powered by [Tokio](https://tokio.rs) for bounded background provider refreshes (max 4 concurrent, 15s per-account timeout).
- **Storage:** SQLite (`rusqlite`) for metadata and historical snapshot storage. OS Keyring (`keyring-rs`) for credential storage.
- **CLI Parser:** [Clap](https://github.com/clap-rs/clap).

---

## Security & Privacy

- **No Remote Telemetry:** LimitLane does not send telemetry, analytics, or usage data to any third-party server.
- **No Plaintext Secret Persistence:** API keys and OAuth tokens are stored strictly inside your operating system's secure credential store.
- **Log Redaction:** All diagnostic and tracing logs automatically redact tokens, authorization headers, and secrets.

---

## License

Distributed under the MIT License. See `LICENSE` for more information.
