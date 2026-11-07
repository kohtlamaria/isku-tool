# Isku tool

A small replacement for the old Roccat software, for the original Roccat Isku
(USB 1E7D:319C). Goal: pick a profile and remap keys, starting with the thumb
keys and the Easy-Shift zone.

- `docs/protocol.md`: what is known about the keyboard, and how sure we are
- `probe/`: scripts that talk to the keyboard
- `src/`: the Rust tool
- `dumps/`: raw results to compare against
- `backups/`: profile backups, kept out of git by default

`src/` is a Rust command-line tool for the keyboard's settings:

- `protocol/` holds the report format, with no USB code, so a Tauri or other
  front end can reuse it later. `mod.rs` has the report table and checksums;
  `keys.rs`, `actions.rs`, `light.rs` and `macros.rs` each cover one area.
- `device.rs` talks to the keyboard. Every write is checked and read back.
- `commands/` has one file per command, and `main.rs` only reads the arguments.
- `set`, `light`, `reset` and `restore` keep the bytes from before each change
  in `backups/changes/`, so `undo` can step back one report at a time.

    cargo test            # needs no keyboard
    cargo run             # shows all 5 profiles
    cargo run -- backup   # also saves them to backups/
    cargo run -- set 2 T1 7           # thumb key on profile 2
    cargo run -- set 2 T1 Ctrl+F5     # letters, F1-F24, Space, Num1, with Ctrl/Shift/Alt/Win
    cargo run -- set 2 Shift+Q Ctrl+1 # Easy-Shift zone and M keys work the same way
    cargo run -- set 2 F5 Ctrl+1      # F1-F12 and media keys: Mute, VolDown, VolUp,
                                      # Play, Prev, Next, WWW, Computer
    cargo run -- set 2 Shift+Q default  # put a zone, F- or media key back to normal
    cargo run -- set 2 T3 profile-cycle # special actions, see below
    cargo run -- restore <folder> 2   # put profile 2 back from a backup
    cargo run -- reset 2              # zone, F-, media, Caps Lock and lighting to factory
    cargo run -- light 2 brightness 3 # brightness or dim 0-5, timeout in minutes
    cargo run -- light 2 default      # factory lighting: 3, dims to 1 after 15 min
    cargo run -- undo                 # put back the last changed report, run again to go further back
    cargo run -- activate <1-5>       # switch the active profile

Nothing that writes to the keyboard belongs in `main` until it has been tried
on a spare profile. Firmware commands are never used.

Key names follow key positions on a US layout. On an Estonian layout some
symbol keys print something else, for example `Minus` is the + key. Letters,
numbers and F-keys are the same.

Special actions for `set`: `off`, `profile-up`, `profile-down`,
`profile-cycle`, `profile-1` to `profile-5`, `easyshift`. Media actions only
work on the media key row, so they are not offered for other keys.

## Licence

GPL version 2 or (at your option) any later version, see `LICENSE`.

The report formats come from two GPL projects, so this tool uses the same
licence:

- the Linux kernel's Roccat Isku driver (`drivers/hid/hid-roccat-isku.c`) by
  Stefan Achatz
- roccat-tools by Stefan Achatz

Roccat and Isku are trademarks of their owners. This project is not affiliated
with them.
