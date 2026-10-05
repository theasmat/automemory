# Installation Guide

AutoMemory is distributed as a single standalone executable with no external runtime dependencies (no Node.js, Python, or external database required).

---

## ⚡ Quick Install

### macOS & Linux (Bash / Zsh)
Run the automated universal installer in your terminal:
```bash
curl -fsSL https://raw.githubusercontent.com/theasmat/automemory-bin/main/install.sh | bash
```

### Windows (PowerShell)
Open PowerShell as your current user and run:
```powershell
irm https://raw.githubusercontent.com/theasmat/automemory-bin/main/install.ps1 | iex
```

The installer detects your operating system and CPU architecture, installs the release binary into your user bin path (`~/.cargo/bin` or `%USERPROFILE%\.cargo\bin`), verifies installation health via `automemory doctor`, and offers to configure all 20 AI coding agents globally.

---

## 📦 Install via Cargo (from Git)

If you have Rust and Cargo installed:
```bash
cargo install --git https://github.com/theasmat/automemory.git --bin automemory
```

Make sure Cargo's bin directory is in your `$PATH`:
- **macOS / Linux**: `export PATH="$HOME/.cargo/bin:$PATH"` in `~/.zshrc` or `~/.bashrc`
- **Windows**: `$env:Path += ";$env:USERPROFILE\.cargo\bin"` in `$PROFILE`

---

## 🛠️ Build from Source

### Prerequisites
- **Rust Toolchain**: Rust 1.80+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **Node.js & pnpm**: Only required if modifying the embedded Web Dashboard (`clients/web`)
- **C Compiler**: `clang` / `gcc` (macOS Xcode CLI tools via `xcode-select --install`, or `build-essential` on Ubuntu/Debian)

### Steps
1. Clone the repository:
   ```bash
   git clone https://github.com/theasmat/automemory.git
   cd automemory
   ```

2. (Optional) Rebuild web dashboard assets:
   ```bash
   cd clients/web
   pnpm install
   pnpm build
   cd ../..
   cp -rf clients/web/dist/* crates/automemory-cli/src/dashboard/assets/
   ```

3. Build the release binary:
   ```bash
   cargo build --release
   ```

4. Install to your local bin path:
   ```bash
   cp target/release/automemory ~/.cargo/bin/
   ```

---

## 🪟 Windows Specific Notes

AutoMemory is fully native on Windows:
1. **PowerShell Execution**: When running the installer, ensure script execution is enabled for the current process:
   ```powershell
   Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
   irm https://raw.githubusercontent.com/theasmat/automemory/main/install.ps1 | iex
   ```
2. **Path Storage**:
   - Master SQLite database: `%USERPROFILE%\.automemory\automemory.db`
   - Agent MCP configs: `%APPDATA%`, `%USERPROFILE%\.codex`, and VS Code `%APPDATA%\Code\User`
3. **WSL2 (Windows Subsystem for Linux)**:
   If you prefer developing inside WSL2, open your WSL Ubuntu shell and use the Linux installer command directly.

---

## 🩺 Verifying Your Installation

Run the system diagnostic check:
```bash
automemory doctor
```

Expected diagnostic output:
```text
=== AutoMemory System Diagnostic (doctor) ===

Checking directories (~/.automemory)... ✓ OK ("/Users/user/.automemory")
Checking SQLite database connectivity & WAL mode... ✓ OK (opened in 2.12ms)
Benchmarking FTS5 search & memory operations... ✓ OK (FTS5 search latency: 1.15ms)
  Active memories: 12, Archived: 2
  Pending event queue: 0 unprocessed events
Snapshotting database backup... ✓ OK
Checking Agent hook configuration... ✓ OK

✓ All health checks passed! AutoMemory is fully operational.
```

---

## 🔄 Updating AutoMemory

To update to the latest release, re-run the universal installer:
- **macOS / Linux**: `curl -fsSL https://raw.githubusercontent.com/theasmat/automemory/main/install.sh | bash`
- **Windows**: `irm https://raw.githubusercontent.com/theasmat/automemory/main/install.ps1 | iex`
- **Cargo**: `cargo install --git https://github.com/theasmat/automemory.git --force`
