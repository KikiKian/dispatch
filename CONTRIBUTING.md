# Contributing to Dispatch

Thanks for your interest in contributing. Dispatch is a Rust TUI process manager focused on CPU core affinity control for Windows and Linux.

## Getting Started

```sh
git clone https://github.com/Klinoxxx/dispatch
cd dispatch
cargo build
```

Rust stable is required. No nightly features are used.

## Project Structure

- `src/main.rs` — core logic: process reading, affinity assignment, eco/performance modes
- `src/tui.rs` — terminal UI

## Areas That Need Work

- **Performance mode** — `eval_process()` and `get_priority_processes()` are stubbed out
- **TUI integration** — mode selection and live process list need to be wired up
- **macOS support** — currently untested/unsupported
- **Error handling** — replace `println!` error paths with proper error propagation

## Platform Notes

Platform-specific affinity code is gated with `#[cfg(target_os)]`. If you add a new OS, follow the same pattern in `direct_process()` and add it to the support table in `README.md`.

- **Windows** — uses `winapi`: `SetProcessAffinityMask`
- **Linux** — uses `nix`: `sched_setaffinity`

## Submitting Changes

1. Fork the repo and create a branch from `main`
2. Keep changes focused — one feature or fix per PR
3. Run `cargo clippy` and `cargo fmt` before opening a PR
4. Describe what the change does and why in the PR body

## Style

- Follow standard Rust idioms; `clippy` is the authority
- No `unsafe` outside of the existing Windows WinAPI block unless strictly necessary

