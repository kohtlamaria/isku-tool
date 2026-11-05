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
`src/protocol.rs` holds the report format and has no USB code, so a Tauri or
other front end can reuse it later. `src/device.rs` can only send the select
command, so it cannot change keys.

    cargo test            # needs no keyboard
    cargo run             # shows all 5 profiles
    cargo run -- backup   # also saves them to backups/

Report formats come from the Linux driver and roccat-tools, both GPL. Check the
licence before publishing this.

Nothing that writes to the keyboard belongs in `main` until it has been tried
on a spare profile. Firmware commands are never used.
