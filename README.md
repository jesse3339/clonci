# clonci

`clonci` is a context-based shell history manager.

Each context stores its own history file, so command history stays isolated between contexts and persists after terminal restart.

Warning: This was "vibecoded".

## Install

Prerequisite: Rust and Cargo are installed.

### Install from this repo

```powershell
cd C:\Projects\clonci
cargo install --path .
```

This installs `clonci` into Cargo's bin directory.

- Windows: `%USERPROFILE%\.cargo\bin`
- Linux/macOS: `~/.cargo/bin`

Make sure that directory is on your `PATH`, then verify:

```text
clonci --help
```

### Build only (no install)

```powershell
cd C:\Projects\clonci
cargo build --release
```

Binary output:

- Windows: `target\release\clonci.exe`
- Linux/macOS: `target/release/clonci`

## Commands

```text
clonci context create <name>
clonci context list
clonci context delete <name>
clonci activate <name> [--shell bash|zsh|pwsh]
clonci enter <name> [--shell bash|zsh|pwsh]
clonci resume [--shell bash|zsh|pwsh]
clonci current
```

Use `enter` to open a new shell process already bound to the context:

```text
clonci enter work --shell bash
clonci enter personal --shell pwsh
clonci resume --shell bash
```

## Optional Shell Helpers

Add helpers in your shell startup file for easier switching.

### Bash


## Storage Location

State is persisted under:

- Windows: `%LOCALAPPDATA%\clonci`
- Linux/macOS: `$XDG_STATE_HOME/clonci` or `~/.local/state/clonci`
