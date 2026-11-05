//! Read-only for now: this program only asks the keyboard for feature reports.

use hidapi::{HidApi, HidDevice};
use isku_tool::protocol::{
    action_bytes, checksum_ok, Report, FEATURE_BUFFER_LEN, PRODUCT_ID, READ_REPORTS,
    SETTINGS_USAGE_PAGE, VENDOR_ID,
};

fn open_settings_interface(api: &HidApi) -> Result<HidDevice, String> {
    let info = api
        .device_list()
        .find(|d| {
            d.vendor_id() == VENDOR_ID
                && d.product_id() == PRODUCT_ID
                && d.usage_page() == SETTINGS_USAGE_PAGE
        })
        .ok_or("Isku settings interface not found")?;
    info.open_device(api).map_err(|e| e.to_string())
}

fn read_report(device: &HidDevice, report: &Report) -> Result<Vec<u8>, String> {
    let mut buffer = vec![0u8; FEATURE_BUFFER_LEN];
    buffer[0] = report.id;
    device
        .get_feature_report(&mut buffer)
        .map_err(|e| e.to_string())?;
    buffer.truncate(report.len);
    Ok(buffer)
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn print_report(device: &HidDevice, report: &Report) {
    println!("0x{:02x} {}", report.id, report.name);
    let bytes = match read_report(device, report) {
        Ok(bytes) => bytes,
        Err(error) => {
            println!("  failed: {error}");
            return;
        }
    };
    if let Some(actions) = report.keys.and_then(|keys| action_bytes(&bytes, keys)) {
        println!("  actions:  {}", hex(actions));
    }
    println!("  raw:      {}", hex(&bytes));
    // Reports 0x04, 0x05 and 0x0f have no checksum.
    if !matches!(report.id, 0x04 | 0x05 | 0x0f) {
        println!("  checksum: {}", if checksum_ok(&bytes) { "ok" } else { "BAD" });
    }
}

fn main() -> Result<(), String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let device = open_settings_interface(&api)?;
    for report in READ_REPORTS {
        print_report(&device, report);
    }
    Ok(())
}
