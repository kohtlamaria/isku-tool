# Isku protocol notes

Confirmed = seen in our own dump. Driver = from the Linux driver, not tested
here. Guess = fits the data, not proven.

## Device

- Confirmed: USB 1E7D:319C, product string "ROCCAT Isku".
- Confirmed: settings interface is `mi_01&col05`, usage page 000B, usage 0000,
  feature report length 2083 (matches the macro report, 0x823).
- Confirmed: Windows gives a read/write handle to this interface.
- Open: whether WebHID allows this interface. It is not an FFxx vendor page.

## Report format

- Confirmed: byte 0 = report ID, byte 1 = length, byte 2 = profile (0-4) on
  key reports.
- Confirmed: last 2 bytes = checksum, the little-endian 16-bit sum of all
  earlier bytes. It matched on all 9 reports checked:
  key_mask, keys_function, keys_easyzone, keys_media, keys_thumbster,
  keys_macro, light, keys_capslock, last_set.
- Confirmed: reads without selecting a profile returned profile 0.
- Confirmed: 0x05 actual_profile returned `05 03 01`, so profile index 1 is active.
- Confirmed: 0x04 control returned `04 01 00`. Driver: value 1 = status OK.

## Selecting a profile

Source: roccat-tools (`isku.h`, `roccat_control.c`). Confirmed on our keyboard:
all 5 profiles and 47 macros read with valid checksums.

- Send feature report `04 <profile 0-4> <request>`, wait 110 ms, then read
  report 0x04 until byte 1 is 1 (OK). 3 means busy. Anything else is an error.
- Requests: key_mask 0x50, keys_function 0x60, light 0x70, keys_capslock 0x90,
  keys_macro 0xa0, keys_easyzone 0xb0, keys_media 0xc0, keys_thumbster 0xd0.
- For a macro, the request is the key index (0x00-0x23).

## Key reports

After the 3-byte header come three arrays: all action bytes, all modifiers, then
all shortcut keys. Capslock has only one action byte.

| ID | Name | Keys | Action bytes seen (profile 0) |
|----|------|------|-------------------------------|
| 0x08 | keys_function | 12 | 3a-45, HID usage codes for F1-F12 |
| 0x09 | keys_easyzone | 20 | HID codes for 1-5, QWERT, ASDFG, ZXCVB |
| 0x0a | keys_media | 8 | 81 82 83 a4 a5 a6 a8 a9 |
| 0x0b | keys_thumbster | 6 | be be be, b9 b9 b9 |
| 0x0d | keys_macro | 10 | be be d5 d6 d7 d8 d9 da db dc |
| 0x13 | keys_capslock | 1 | 8c |

Key order, from roccat-tools: thumbster T1-T3 then Easy-Shift T1-T3, macro keys
M1-M5 then Easy-Shift M1-M5.

Action bytes, from roccat-tools: be macro, b9 quicklaunch, b8 shortcut,
8c Easy-Shift, 00 disabled, ff Easy-Shift, d3-d7 Shift+1-5, d8-dc Ctrl+1-5.
Values below 0x80 are treated as plain HID key codes, which matches our dump.

Shortcut modifier bits: 1 Shift, 2 Ctrl, 3 Alt, 4 Win.

Macro indexes: M1-M5 00-04, Easy-Shift M 05-09, Easy-Shift zone 0a-1d,
T1-T3 1e-20, Easy-Shift T 21-23.

## Macro report (0x0e, 2083 bytes)

Offsets from roccat-tools: 0 id, 1 size (16-bit), 3 profile, 4 key index, 5 type,
6 loop, 31 macroset name (24), 55 macro name (24), 79 count (16-bit),
81 keystrokes (500 of key, action, period 16-bit), 2081 checksum.
Keystroke action: 1 press, 2 release.

## Other reports

- 0x14 last_set bytes 2-5: Unix time per roccat-tools. Ours reads
  2026-08-27 15:10:03 UTC.
- 0x0f info: byte 2 firmware version, byte 3 DFU version.
- 0x11 reset, 0x1b and 0x1c firmware: never used here.

## Writing

Source: roccat-tools. Not yet tried on our keyboard.

- A key report is written as a whole: id, size, profile, the three arrays,
  checksum. Then wait for status OK, as with select.
- 0x05 actual_profile is written as `05 03 <profile>`.

Confirmed: the old software stored number bindings as macros with a press and a
release. For a Shortcut (b8) binding, roccat-tools stores the key and modifier in
the arrays, with no macro needed.

Confirmed on Profile 5: writing T1 as a b8 shortcut for key 7 was accepted,
read back the same, and typed 7. Holding it gives one 7, the same as the old
macros, so neither repeats while held.

Tried on Profile 5: a plain HID code (24, key 7) as T1's action was accepted
and read back, but T1 then did nothing. Thumb keys only take Roccat actions, so
they cannot hold a key down.

Writes are now allowed to any profile, after a backup.

## Next questions

1. Easy-Shift zone: does a b8 shortcut work there the same way?
