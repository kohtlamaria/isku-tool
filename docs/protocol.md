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
- Driver: control has to be written before reading a chosen profile.

## Key reports

Layout after the 3-byte header, all confirmed by size and contents except where
marked: one action byte per key, then two zeroed bytes per key.

| ID | Name | Keys | Action bytes seen (profile 0) |
|----|------|------|-------------------------------|
| 0x08 | keys_function | 12 | 3a-45, HID usage codes for F1-F12 |
| 0x09 | keys_easyzone | 20 | HID codes for 1-5, QWERT, ASDFG, ZXCVB |
| 0x0a | keys_media | 8 | 81 82 83 a4 a5 a6 a8 a9 |
| 0x0b | keys_thumbster | 6 | be be be, b9 b9 b9 |
| 0x0d | keys_macro | 10 | be be d5 d6 d7 d8 d9 da db dc |
| 0x13 | keys_capslock | 1 | 8c |

Guesses:

- 0x0b: first 3 = T1-T3, next 3 = their Easy-Shift versions.
- 0x0d: first 5 = M1-M5, next 5 = their Easy-Shift versions.
- 0xbe may mean "macro". Report 0x0e, which holds macros, has not been read.
- 0x8c may mean Easy-Shift.
- The per-key zero pairs may hold modifiers or parameters.
- 0x14 last_set bytes 2-5 may be a Unix time: 2026-08-27 15:10:03 UTC.

## Next questions

1. Read all 5 profiles (needs the control select write).
2. Read 0x0e to see what 0xbe points at.
3. Change one known key on a spare profile and compare the bytes.
