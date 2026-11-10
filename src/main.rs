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
//! cargo run -- set 2 CapsLock off   Caps Lock: easyshift, capslock, off or default
//! cargo run -- macro 2 T1 "Ctrl+C, wait 50, Ctrl+V"  make a key play a macro
//! cargo run -- copy 5 2             copy profile 5's keys, lighting and macros to profile 2
//! cargo run -- export 2 p2.txt      save profile 2 as a text file you can edit
//! cargo run -- import p2.txt 5      set profile 5 to what a text file says
//! cargo run -- undo                 put back the last command's changes
//! cargo run -- activate <1-5>       switch the active profile

mod commands;

use commands::{
    cleanup, light, macros, new_backup_folder, profile_file, profiles, set, show, undo,
};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = run(&args);
    // Commands that save backups or change files tidy up after themselves.
    let saves_files = matches!(
        args.first(),
        Some(
            &("backup"
                | "set"
                | "macro"
                | "light"
                | "reset"
                | "restore"
                | "copy"
                | "import"
                | "undo")
        )
    );
    if result.is_ok() && saves_files {
        cleanup::cleanup();
    }
    result
}

fn run(args: &[&str]) -> Result<(), String> {
    match args {
        [] => show::show(None),
        ["backup"] => show::show(Some(&new_backup_folder()?)),
        ["set", profile, label, value] => set::set_key(profile, label, value),
        ["macro", profile, label, text] => macros::set_macro(profile, label, text),
        ["restore", folder, profile] => profiles::restore(folder, profile),
        ["activate", number] => profiles::activate(number),
        ["copy", from, to] => profiles::copy(from, to),
        ["export", profile, file] => profile_file::export(profile, file),
        ["import", file, profile] => profile_file::import(file, profile),
        ["reset", number] => profiles::reset(number),
        ["undo"] => undo::undo(),
        ["light", number, setting] => light::set_light(number, setting, None),
        ["light", number, setting, value] => light::set_light(number, setting, Some(value)),
        _ => Err(
            "use: backup, set <1-5> T1 7, macro <1-5> T1 \"Ctrl+C\", restore <folder> <1-5>, reset <1-5>, undo, light <1-5> ..., copy <from> <to>, export <1-5> <file>, import <file> <1-5> or activate <1-5>".to_string(),
        ),
    }
}
