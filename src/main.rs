//! cargo run                         show all 5 profiles
//! cargo run -- backup               show them and save them to backups/
//! cargo run -- set 2 T1 7           set a thumb key on profile 2
//! cargo run -- set 2 T1 Ctrl+F5     keys can be letters, F-keys, Space and so on
//! cargo run -- set 2 Shift+Q Ctrl+1  Easy-Shift zone keys and M keys work the same way
//! cargo run -- set 2 F5 Ctrl+1      F1-F12 and media keys (Mute, VolUp...) too
//! cargo run -- set 2 Shift+Q default put a zone, F- or media key back to normal
//! cargo run -- set 2 T3 profile-cycle  actions: off, profile-1, easyshift...
//! cargo run -- restore <folder> 2   put profile 2 back from a backup
//! cargo run -- reset 2             put profile 2's keys and lighting back to factory settings
//! cargo run -- light 2 brightness 3 lighting: brightness, dim (0-5) or timeout (min)
//! cargo run -- light 2 default      factory lighting
//! cargo run -- undo                 put back the last set or light change
//! cargo run -- activate <1-5>       switch the active profile

mod commands;

use commands::{light, new_backup_folder, profiles, set, show, undo};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] => show::show(None),
        ["backup"] => show::show(Some(&new_backup_folder()?)),
        ["set", profile, label, value] => set::set_key(profile, label, value),
        ["restore", folder, profile] => profiles::restore(folder, profile),
        ["activate", number] => profiles::activate(number),
        ["reset", number] => profiles::reset(number),
        ["undo"] => undo::undo(),
        ["light", number, setting] => light::set_light(number, setting, None),
        ["light", number, setting, value] => light::set_light(number, setting, Some(value)),
        _ => Err(
            "use: backup, set <1-5> T1 7, restore <folder> <1-5>, reset <1-5>, undo, light <1-5> ... or activate <1-5>".to_string(),
        ),
    }
}
