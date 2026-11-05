# Isku tool

A small replacement for the old Roccat software, for the original Roccat Isku
(USB 1E7D:319C). Goal: pick a profile and remap keys, starting with the thumb
keys and the Easy-Shift zone.

- `docs/protocol.md`: what is known about the keyboard, and how sure we are
- `probe/`: scripts that talk to the keyboard
- `src/`: the Rust tool
- `dumps/`: raw results to compare against
- `backups/`: profile backups, kept out of git by default

`src/` is a Rust command-line tool that reads the settings from the keyboard.
It only reads for now. `src/protocol.rs` holds the report format and has no USB
code, so a Tauri or other front end can reuse it later.

    cargo test   # needs no keyboard
    cargo run    # reads the keyboard

Nothing that writes to the keyboard belongs in `main` until it has been tried
on a spare profile. Firmware commands are never used.
