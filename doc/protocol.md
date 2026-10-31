# Protocol notes

Plain-language description of what `src/` implements so far. Every statement
marked **observation** is enforced by a test under `tests/`; open questions
are collected at the end and decided on evidence.

---

## 1. CRC-16 (`src/crc.rs`, `tests/crc.rs`)

CRC-16 is the general-purpose integrity check of this protocol. It protects
the modem table files byte-for-byte (section 2) and is expected to appear
again at radio-frame level; that later reuse is currently unconfirmed (see
open questions).

The implementation lives in `src/crc.rs` as `vara_native::crc::crc16`.

### 1.1 Parameters

| Parameter | Value |
|---|---|
| Width | 16 bits |
| Polynomial | `0x1021` (CCITT form: x^16 + x^12 + x^5 + 1) |
| Initial value | `0xFFFF` |
| Reflect input | no |
| Reflect output | no |
| Final XOR | `0xFFFF` |
| Check value | `crc16(b"123456789") = 0xD64E` |
| Common name | CRC-16/GENIBUS |

Because neither direction is reflected, each byte enters the register
through the high half of the current state.

### 1.2 Algorithm

Straightforward bitwise definition (the same pseudocode is cross-checked in
`tests/crc.rs` by an implementation that shares no code with `src/`):

```text
function crc16(data):
    r := 0xFFFF
    for byte b in data:
        r := r XOR (b shifted left by 8)
        repeat 8 times:
            if bit 15 of r is set:
                r := (r shifted left by 1) XOR 0x1021
            else:
                r := r shifted left by 1
    return r XOR 0xFFFF
```

### 1.3 Implementation notes

- `src/crc.rs` builds the classic 256-entry lookup table at compile time
  (`const fn`): the runtime cost is one table index plus one XOR per byte,
  with no allocation and no dependencies. The crate carries zero runtime
  dependencies (`Cargo.toml` `[dependencies]` is empty).
- The function returns the raw `u16` after the final XOR. Callers that
  write a CRC into a byte stream must serialize it **big-endian**
  (`u16::to_be_bytes`), as the table loader does.

### 1.4 Behavior reference

| Input | Result |
|---|---|
| `[]` (empty) | `0x0000` (init XOR xorout, no data consumed) |
| `123456789` | `0xD64E` (standard check value) |
| `X` | `0xC5F2` |
| `AB` | `0xB48B` |

`tests/crc.rs` additionally runs the table-driven implementation and an
independent bitwise reference on real table-file bytes; an error in either
the table or the bit loop is caught immediately, because the two reference
implementations must agree on every input.

### 1.5 Scope quirk: the final data byte is not covered

When this CRC protects its table files, the computing loop **stops one byte
short**: the last data byte preceding the stored CRC is not fed into the
register. For a block ending at offset `n`, the stored value is
`crc16(bytes[..n-1])`, *not* `crc16(bytes[..n])`.

This is treated as observed behavior to be reproduced, not as a bug to fix:
compatibility means accepting and rejecting exactly the byte sequences the
original software accepts and rejects. Feeding the final byte changes the
result — for the first 16 bytes of a table file, the 15-byte-scope and
16-byte-scope results differ:

```text
first 16 bytes of a table file                 -> 0x1581
same bytes, scope shortened by one             -> 0xD4E2
```

`tests/dat.rs::last_data_byte_is_not_covered_by_crc` pins this down from the
outside with a mutation test: flipping only the last byte of the table block
must still yield `Ok`, while flipping any covered byte must fail the check
(`rejects_bad_crc`).

---

## 2. Modem table files (`src/dat.rs`, `tests/dat.rs`)

Per-bandwidth modem constants live in three table files, one per bandwidth.
Every file is accepted or rejected as a whole — there is no partial loading.

All numbers in this section are byte offsets unless stated otherwise.

### 2.1 Files and the size rule

| File | Bandwidth | Total size (bytes) |
|---|---|---|
| `VARAHF500.dat` | 500 Hz | 734 771 (`0xB3633`) |
| `VARAHF2300.dat` | 2300 Hz | 720 716 (`0xAFF4C`) |
| `VARAHF2750.dat` | 2750 Hz | 872 206 (`0xD4F0E`) |

The total sizes are part of the acceptance rule: a file must match one of
these numbers **exactly**; one byte fewer or more is rejected outright
(`tests/dat.rs::file_sizes_match_vara_loader`, `rejects_wrong_size`).

### 2.2 Layout

```text
offset 0x000000 +-----------------------------------+
                | table block                       |  see 2.3
                | length = n (per bandwidth)        |
                +-----------------------------------+
offset n        | CRC-16, big-endian                |  2 bytes, see section 1
                +-----------------------------------+
offset n+2      | extra block                       |  VARAHF500.dat ONLY
                | length = total_size − n − 2       |
                +-----------------------------------+
```

Per-bandwidth block boundaries:

| Bandwidth | Table block length `n` | CRC offset | Extra block length |
|---|---|---|---|
| 500 Hz | `0x2DC00` (187 392) | `0x2DC00` | 547 377 bytes, no checksum |
| 2300 Hz | `0xAFF4A` (720 714) | `0xAFF4A` | — |
| 2750 Hz | `0xD4F0C` (872 204) | `0xD4F0C` | — |

- For the 2300 and 2750 files the table block fills the file up to its final
  two bytes, which hold the CRC.
- The 500 Hz file is special: a complete first block *with* a CRC, followed
  by a second block of 547 377 bytes that carries **no** checksum of its own
  (`only_500_has_extra_block`).

### 2.3 Table block

- **Word structure.** The block (CRC bytes excluded) is a sequence of
  **big-endian 16-bit words**; the block length is always even
  (`words_are_big_endian`).
- **Leading structure shared by all three files** (`real_files_start_with_two_permutations`):
  1. Words `0..300` form a permutation of `0..299` (each value once).
  2. Words `300..396` form a permutation of `0..95` (each value once).
  These leading permutations are identical across all three bandwidth files
  and are presumed bandwidth-independent interleavers; confirming their
  radio role is deferred to recordings.
  Trailing the permutations, all three blocks additionally end the covered
  region with two zero bytes (`0x0000` word) directly before the CRC.
- **Beyond the permutations** the three files diverge and carry per-bandwidth
  data. Part of it is now identified: one **pilot mask** per PSK/QAM speed
  level, in level order (section 3.4). The words between the masks are not
  yet interpreted; the loader keeps treating the whole block as opaque words.
- The final data byte (position `n−1`) is inside `words` but outside the
  checksum scope (section 1.5).

### 2.4 Checksum rule

The two bytes at offset `n` store the CRC-16 of section 1, serialized
**big-endian**, computed over the table block **minus its last byte**:

```text
stored_crc(bytes, n) = crc16(bytes[0 .. n-1])
```

Concretely verified values (real files):

| File | Stored CRC | Match |
|---|---|---|
| `VARAHF500.dat` | `0x1B72` | yes |
| `VARAHF2300.dat` | `0xFBAC` | yes |
| `VARAHF2750.dat` | `0x3DB1` | yes |

On a size or checksum mismatch the original loader exits silently; the Rust
loader rejects instead: `DatError::WrongSize { expected, actual }` or
`DatError::BadCrc { stored, computed }` — the idiomatic equivalents the test
suite exercises.

### 2.5 Extra block (500 Hz only)

- Length: 547 377 bytes = `total_size − 0x2DC00 − 2`.
- Not covered by any checksum, no permutation structure known.
- Copied out by the original loader as one separate array; preserved here as
  `ModemTables::extra` rather than merged into `words`.
- Purpose currently unknown (open question 2).

### 2.6 API summary

```rust
use vara_native::dat::{parse, Bandwidth, DatError};

let bytes = std::fs::read("VARAHF2300.dat")?;
let tables = parse(Bandwidth::Hz2300, &bytes)?;
assert_eq!(tables.words.len(), 0xAFF4A / 2);
```

`parse` enforces, in order:

1. `bytes.len() == bandwidth.file_size()`, otherwise `WrongSize`;
2. `crc16(&bytes[..n-1]) == u16::from_be_bytes(bytes[n..n+2])`, otherwise
   `BadCrc`;
3. on success: the table block as `words: Vec<u16>` (big-endian decoded) and
   any beyond-CRC bytes raw as `extra: Vec<u8>`.

---

## 3. Speed levels (`src/modes.rs`, `tests/modes.rs`)

Each bandwidth offers a ladder of speed levels, from robust FSK at the bottom
to 32QAM OFDM at the top. `vara_native::modes::levels(bandwidth)` returns the
ladder as a static slice of `SpeedLevel`, level 1 first.

### 3.1 Where the numbers come from

Two independent sources, each covering what the other cannot:

| Source | Provides |
|---|---|
| Speed level table published by the VARA author (image shipped with the software, v4.3.0) | per level: nominal baud, carriers, modulation, net bit rate |
| Mode table the original software builds in memory at startup | per level: bits per symbol, frame length, pilots per carrier, single-carrier frequencies |

The two agree on every overlapping field (carriers and modulation of all 46
levels). The second source is in turn confirmed by the table files: its frame
sizes predict the pilot masks of section 3.4 exactly, in order, for all three
bandwidths. A field enters `SpeedLevel` only when it is backed this way;
fields whose meaning is still open stay out of the API (section 3.6).

### 3.2 The ladders

`carriers` means FSK tones on FSK levels and subcarriers elsewhere. Rates are
the author's rounded figures (`nominal_baud`, `net_bps`).

**500 Hz — 13 levels**

| Level | Modulation | Carriers | Baud | Net bps | Frame symbols | Pilots / carrier |
|---|---|---|---|---|---|---|
| 1 | FSK | 11 | 23 | 18 | — | — |
| 2 | FSK | 11 | 47 | 41 | — | — |
| 3 | FSK | 11 | 47 | 61 | — | — |
| 4 | BPSK | 2 | 94 | 88 | 393 | 18 |
| 5 | QPSK | 2 | 94 | 177 | 394 | 18 |
| 6 | QPSK | 3 | 94 | 270 | 375 | 12 |
| 7–9 | QPSK | 11 | 42 | 441 / 588 / 705 | 149 | 4 |
| 10–11 | 8PSK | 11 | 42 | 884 / 1060 | 149 | 4 |
| 12 | 16QAM | 11 | 42 | 1286 | 154 | 4 |
| 13 | 32QAM | 11 | 42 | 1543 | 154 | 4 |

**2300 Hz — 16 levels**

| Level | Modulation | Carriers | Baud | Net bps | Frame symbols | Pilots / carrier |
|---|---|---|---|---|---|---|
| 1 | FSK | 32 | 23 | 18 | — | — |
| 2–3 | FSK | 16 | 47 | 41 / 82 | — | — |
| 4 | FSK | 16 | 94 | 175 | — | — |
| 5 | QPSK | 3 | 94 | 270 | 375 | 12 |
| 6 | QPSK | 4 | 94 | 363 | 364 | 9 |
| 7 | QPSK | 6 | 94 | 549 | 354 | 6 |
| 8 | QPSK | 8 | 94 | 735 | 351 | 5 |
| 9 | QPSK | 10 | 94 | 922 | 348 | 4 |
| 10–12 | QPSK | 49 | 42 | 2011 / 2682 / 3219 | 149 | 1 |
| 13–14 | 8PSK | 49 | 42 | 4025 / 4830 | 149 | 1 |
| 15 | 16QAM | 49 | 42 | 5872 | 154 | 1 |
| 16 | 32QAM | 49 | 42 | 7050 | 154 | 1 |

**2750 Hz — 17 levels**

| Level | Modulation | Carriers | Baud | Net bps | Frame symbols | Pilots / carrier |
|---|---|---|---|---|---|---|
| 1 | FSK | 40 | 23 | 18 | — | — |
| 2–3 | FSK | 20 | 47 | 41 / 82 | — | — |
| 4 | FSK | 20 | 94 | 175 | — | — |
| 5–9 | QPSK | 3 / 4 / 6 / 8 / 10 | 94 | as 2300 L5–9 | as 2300 L5–9 | as 2300 L5–9 |
| 10 | QPSK | 13 | 94 | 1203 | 348 | 3 |
| 11–13 | QPSK | 59 | 42 | 2423 / 3230 / 3877 | 149 | 1 |
| 14–15 | 8PSK | 59 | 42 | 4848 / 5817 | 149 | 1 |
| 16 | 16QAM | 59 | 42 | 7074 | 154 | 1 |
| 17 | 32QAM | 59 | 42 | 8489 | 154 | 1 |

Tests: `level_counts`, `levels_are_numbered_from_one`,
`matches_published_speed_table` (the author's table is restated in the test
file as an independent copy), `net_rate_grows_with_level`.

### 3.3 Bits per symbol

| Modulation | Bits per symbol |
|---|---|
| FSK | — (not a constellation) |
| BPSK | 1 |
| QPSK | 2 |
| 8PSK | 3 |
| 16QAM | 4 |
| 32QAM | 5 |

These are the values the original mode table stores per level; they match
the modulation names of the published table on every PSK/QAM level
(`bits_per_symbol`).

### 3.4 Frame maps and pilot masks

Every PSK/QAM level transmits frames laid out as a grid of
`frame_symbols × carriers` cells. Some cells carry known pilot symbols:
`pilots_per_carrier × carriers` of them per frame.

- `SpeedLevel::frame_cells()` = `frame_symbols × carriers`
- `SpeedLevel::pilots()` = `pilots_per_carrier × carriers`
- Both are `None` on FSK levels (`fsk_levels_carry_no_frame_map`).

The original mode table stores the frame length **plus one**
(e.g. 150 for a 149-symbol frame); `frame_symbols` holds the real length.

**Observation.** The table files contain one mask per PSK/QAM level, in level
order: a run of 16-bit words that are all `0` or `1`, whose length equals
`frame_cells()` and whose count of ones equals `pilots()`. Example, 2300 Hz:

| Level | Mask length | Ones | = frame symbols × carriers | = pilots / carrier × carriers |
|---|---|---|---|---|
| 5 | 1125 | 36 | 375 × 3 | 12 × 3 |
| 6 | 1456 | 36 | 364 × 4 | 9 × 4 |
| 7 | 2124 | 36 | 354 × 6 | 6 × 6 |
| 8 | 2808 | 40 | 351 × 8 | 5 × 8 |
| 9 | 3480 | 40 | 348 × 10 | 4 × 10 |
| 10–14 | 7301 | 49 | 149 × 49 | 1 × 49 |
| 15–16 | 7546 | 49 | 154 × 49 | 1 × 49 |

The pilot count stays near-constant (36–49 cells) while the grid grows, so
the pilot density falls as levels rise — expected for a design that spends
pilots on weak signals and capacity on strong ones.

Tests: `frame_maps_match_dat_masks` (self-contained, expected masks restated),
`real_files_hold_one_mask_per_psk_level` (scans the real files; needs
`VARA_DIR`).

**Service frames.** After the level masks the files hold masks that belong to
no published level:

| Mask | Grid | Files |
|---|---|---|
| 550 cells, 44 pilots | 11 carriers × 50 symbols, 4 pilots each | 500 (twice), 2300 (once) |
| 900 cells, 36 pilots | 2 carriers × 450 symbols, 18 pilots each | 500 |

Both grids also exist as extra entries in the original mode table. They are
presumed to be control frames (calls, acknowledgements, 500 Hz compatibility);
see open question 4. `real_files_hold_one_mask_per_psk_level` accepts exactly
these two shapes after the level masks and nothing else.

### 3.5 Single-carrier frequencies

The 94-baud PSK levels do not use OFDM: they send a handful of independent
tones at fixed audio frequencies, listed in `carrier_freqs_hz`. The OFDM
(42 baud) and FSK levels leave the list empty.

| Bandwidth | Level | Carriers (Hz) | Spacing |
|---|---|---|---|
| 500 | 4, 5 | 1350, 1650 | 300 |
| 500 | 6 | 1320, 1500, 1680 | 180 |
| 2300 | 5 | 900, 1500, 2100 | 600 |
| 2300 | 6 | 750 … 2250 | 500 |
| 2300 | 7 | 700 … 2300 | 320 |
| 2300 | 8 | 702 … 2298 | 228 |
| 2300 | 9 | 555 … 2445 | 210 |
| 2750 | 5 | 600, 1500, 2400 | 900 |
| 2750 | 6 | 510 … 2490 | 660 |
| 2750 | 7 | 450 … 2550 | 420 |
| 2750 | 8 | 380 … 2620 | 320 |
| 2750 | 9 | 375 … 2625 | 250 |
| 2750 | 10 | 240 … 2760 | 210 |

Every set is evenly spaced and centred on **1500 Hz** — the audio centre the
author's guide gives for VARA (USB dial + 1500 Hz)
(`single_carrier_frequencies_are_centred_and_even`).

### 3.6 Deliberately left out

The original mode table holds more per-level fields than `SpeedLevel`
exposes. They stay out until a recording or a second source fixes their
meaning:

- a per-level SNR figure rising from about −13.6 dB (level 1) to +18 dB
  (top level) — very likely the threshold for choosing a level;
- FSK tone placement (first/last FFT bin) and samples-per-symbol figures that
  point to a 3000 Hz complex baseband; this is a hypothesis until checked
  against audio;
- several per-level scaling constants of unknown role.

---

## 4. Running the test suite

```sh
cargo test                # self-contained: synthetic files only
VARA_DIR=/path/to/installed/VARA cargo test   # additionally checks real files
```

When `VARA_DIR` is unset, the tests that read real table files are skipped
(`tests/dat.rs` returns early). The repository itself contains no vendor
data — parsers, not tables.

---

## 5. Open questions

1. Do radio frames use this same CRC, and if so with the same off-by-one
   scope? Decide from recordings with known payloads. `src/dat.rs` stays
   free of that assumption so the frame layer can decide on evidence.
2. What is the extra block of `VARAHF500.dat` for? It is intact, not covered
   by any checksum, and unaccounted for; next step is recording evidence.
3. Are the leading permutations really common-frame interleavers, and what
   is the role of the two zero words that precede the CRC in every block?
4. What are the two service frame grids (11 × 50 and 2 × 450) used for, and
   which audio frequencies do their carriers occupy?
5. What lies between the pilot masks in the table files? The strongest lead
   is forward error correction structure: its index ranges track the number
   of coded bits per frame of each level. Not asserted anywhere yet.
6. Exact symbol rates and OFDM subcarrier frequencies: the published baud
   figures are rounded; the 3000 Hz baseband hypothesis (section 3.6) would
   make them 23.4375 / 46.875 / 93.75 / 41.67 baud. Decide from recordings.
