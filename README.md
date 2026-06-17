# Dispatch

Dispatch is a process manager that lets you control which CPU cores your running processes are pinned to and it is built for laptops that don't have built-in eco or performance modes. Instead of letting the OS scatter work across all available cores, you can assign processes to specific cores, keeping background tasks out of the way, letting idle cores sleep, and giving your most important work dedicated resources.

## Modes

### Eco Mode
Restricts all processes to half of the available physical cores. Useful for reducing heat and power consumption while keeping the system responsive.

### Performance Mode
_(in progress)_ Prioritizes high-value processes by directing them to dedicated cores first, then distributes remaining processes based on evaluation scores.

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
# not yet implemented — TUI and mode selection coming soon
cargo run
```

---

*"Built for efficiency, founded by simplicity."*

## Tickets

- [ ] Finish each mode (performance, gaming, eco)
    - [ ] Maybe multi-threading processing
    - [ ] better algo 
- [ ] Daemon integration
- [ ] Testing implementation for linux
    - [ ] likely on WSL 

