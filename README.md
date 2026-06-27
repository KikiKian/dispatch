# Dispatch

Dispatch is a process manager that lets you control which CPU cores your running processes are pinned to and it is built for laptops that don't have built-in eco or performance modes. Instead of letting the OS scatter work across all available cores, you can assign processes to specific cores, keeping background tasks out of the way, letting idle cores sleep, and giving your most important work dedicated resources.

## Modes

### Eco Mode
Restricts all processes to half of the available physical cores. Useful for reducing heat and power consumption while keeping the system responsive.

### Performance Mode
Prioritizes high-value processes by directing them to dedicated cores first, then distributes remaining processes based on evaluation scores.

## Platform Support

| Platform | Status |
|----------|--------|
| Windows  | Supported (via `winapi`) |
| Linux    | Supported (via `nix`) |

## Building

```sh
cargo build --release
```

## Usage

```sh
cargo run
```
## Installation

Requires [Rust](https://www.rust-lang.org/tools/install) (stable toolchain).

### Linux / macOS

```bash
git clone https://github.com/kikikian/dispatch.git
cd dispatch
cargo install --path .
```

This installs the `dispatch` binary to `~/.cargo/bin`. If `dispatch` isn't found after install, add it to your PATH:

```bash
echo 'export PATH="$PATH:$HOME/.cargo/bin"' >> ~/.bashrc
source ~/.bashrc
```

(If you're using zsh instead of bash, use `~/.zshrc` instead.)

Verify:
```bash
dispatch read
```

### Windows (PowerShell)

```powershell
git clone https://github.com/kikikian/dispatch.git
cd dispatch
cargo install --path .
```

This installs `dispatch.exe` to `%USERPROFILE%\.cargo\bin`. The Rust installer adds this to PATH automatically — if it's missing, add it manually:

1. Search "Environment Variables" in the Start menu → Edit environment variables for your account
2. Edit the `Path` variable, add: `%USERPROFILE%\.cargo\bin`
3. Restart PowerShell

Verify:
```powershell
dispatch read
```

---

**Note:** `dispatch` manages system processes (affinity, suspend, kill), so some commands may require elevated privileges:
- Linux/macOS: run with `sudo` if you hit permission errors
- Windows: run PowerShell as Administrator if you hit permission errors
---

*"Built for efficiency, founded by simplicity."*

## Tickets

- [x] Finish each mode (performance, gaming, eco)
    - [x] Maybe multi-threading processing
    - [x] better algo 
- [ ] Daemon integration
- [x] Testing implementation for linux
    - [x] likely on WSL 

