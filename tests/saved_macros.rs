//! Checks every macro saved in a full backup in backups/ against the macro
//! builder. backups/ is kept locally and out of git, so this only runs when
//! asked: cargo test -- --ignored

use isku_tool::protocol::{
    build_macro, macro_file_name, parse_macro, parse_macro_file_name, valid_macro_report,
};
use std::fs;

#[test]
#[ignore = "needs the local backups/ folder"]
fn every_saved_macro_rebuilds_byte_for_byte() {
    let mut checked = 0;
    for folder in fs::read_dir("backups").expect("backups/ folder") {
        let folder = folder.unwrap().path();
        let is_backup = folder
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("backup-"));
        if !folder.is_dir() || !is_backup {
            continue;
        }
        for file in fs::read_dir(&folder).unwrap() {
            let path = file.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let Some((profile, key_index, false)) = parse_macro_file_name(&name) else {
                continue;
            };
            let bytes = fs::read(&path).unwrap();
            assert_eq!(name, macro_file_name(profile, key_index));
            assert!(
                valid_macro_report(profile, key_index, &bytes),
                "{}",
                path.display()
            );
            let parsed = parse_macro(&bytes).expect("parses");
            let rebuilt = build_macro(profile, key_index, &parsed).expect("rebuilds");
            assert!(rebuilt == bytes, "{} rebuilds differently", path.display());
            checked += 1;
        }
    }
    assert!(checked > 0, "no saved macros found");
    println!("{checked} saved macros rebuild byte for byte");
}
