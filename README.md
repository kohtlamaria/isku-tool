# Isku tool

A small replacement for the old Roccat software, for the original Roccat Isku
(USB 1E7D:319C). Goal: pick a profile and remap keys, starting with the thumb
keys and the Easy-Shift zone.

- `docs/protocol.md`: what is known about the keyboard, and how sure we are
- `probe/`: scripts that talk to the keyboard
- `dumps/`: raw results to compare against
- `backups/`: profile backups, kept out of git by default

The app itself is not started yet. The stack is still open: WebHID + React, or
Tauri (React + Rust).

Nothing that writes to the keyboard belongs in `main` until it has been tried
on a spare profile. Firmware commands are never used.
