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
  These are the two interleavers of **level 1** (section 2.7). They are
  identical across the three files because level 1 has the same frame
  payload in every bandwidth (K = 96, N = 300).
  Trailing the permutations, all three blocks additionally end the covered
  region with two zero bytes (`0x0000` word) directly before the CRC.
- **Beyond the permutations** the three files diverge and carry per-bandwidth
  data. The whole block is now accounted for: it is a sequence of 17
  **records**, one per speed level followed by service frames (section 2.7).
  `parse` still returns it as plain words; `ModemTables::records()` walks it.
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
- It belongs to the original software's licensing, not to the modem, and is
  deliberately left undecoded (open question 2).

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

### 2.7 Records (`ModemTables::records`, `tests/dat_records.rs`)

The table block is a sequence of **17 records**: one per speed level, level 1
first, then service frames (`record_shapes`). Each record is, in order:

| Part | Length (words) | Content |
|---|---|---|
| Channel interleaver | N | permutation of `0..N` (coded bits) |
| Information interleaver | K | permutation of `0..K` (information bits) |
| PSK/QAM: pilot positions | carriers × pilots per carrier | symbol numbers of the pilots, per carrier |
| PSK/QAM: pilot mask | carriers × frame symbols | 0/1, carrier by carrier |
| FSK: tone map | frame symbols | base FFT bin of every symbol |
| FSK: pilot list | 24 | symbol numbers of the pilot symbols |
| FSK: pairs | 2 × frame symbols | (pilot flag, pilot value) per symbol |

K and N per level are in section 3.7. After the levels come the service
frames: an 11-carrier × 50-symbol PSK frame (500 Hz twice, 2300 Hz once),
plus in the 500 Hz file a 2 × 450 PSK frame and a 92-symbol FSK frame.

**Observations**, all checked by tests:

- the record layout accounts for every word of all three blocks, with nothing
  left over (`layout_fills_table_block_exactly`, `real_files_walk_cleanly`);
- every interleaver is a permutation of the stated length, and every mask is
  0/1 — anything else is rejected (`rejects_non_permutation`,
  `rejects_non_binary_mask`);
- the pilot positions of a carrier are exactly the 1s of its mask, counted
  from symbol 2: mask entry `k` is symbol `k + 2`
  (`real_pilot_positions_are_the_mask_ones`).

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
| 1 | FSK | 11 | 23 | 18 | 124 | — |
| 2 | FSK | 11 | 47 | 41 | 228 | — |
| 3 | FSK | 11 | 47 | 61 | 226 | — |
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
| 1 | FSK | 32 | 23 | 18 | 124 | — |
| 2–3 | FSK | 16 | 47 | 41 / 82 | 228 | — |
| 4 | FSK | 16 | 94 | 175 | 395 | — |
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
| 1 | FSK | 40 | 23 | 18 | 124 | — |
| 2–3 | FSK | 20 | 47 | 41 / 82 | 228 | — |
| 4 | FSK | 20 | 94 | 175 | 395 | — |
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
- Both are `None` on FSK levels, which have a frame length but no pilot mask
  (`fsk_levels_carry_no_pilot_mask`; FSK frame lengths in `fsk_frame_lengths`).

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
- several per-level scaling constants of unknown role.

FSK tone placement and the exact symbol rates were in this list; they are now
in `SpeedLevel` (sections 3.7 and 3.8).

### 3.7 Frame payload and exact rates (`tests/modes.rs`)

Every level carries a frame of **K information bits** coded into **N coded
bits** (`info_bits`, `coded_bits`): K and N are the lengths of the level's two
interleavers (section 2.7). The symbol length is a whole number of samples of
a 3000 Hz baseband (`samples_per_symbol`):

| Nominal baud | Samples at 3000 Hz | Exact rate |
|---|---|---|
| 23 | 128 | 23.4375 |
| 47 | 64 | 46.875 |
| 94 | 32 | 93.75 |
| 42 | 72 (64 + 8 prefix) | 41.667 |

**Observation.** `K × symbol_rate / frame_symbols` reproduces every one of the
46 published net rates to within 2 bit/s, the rounding of the published figure
(`info_bits_reproduce_published_net_rates`): the published rate counts K bits
per frame of `frame_symbols` symbols. Whatever else goes on air around a frame
(lead-in, gaps) is outside that figure.

The code rate (`code_rate`) of each level is 1/3, 1/2, 2/3, 4/5 or 5/6, and N
follows the turbo rule of section 4.3 (`coded_length_follows_turbo_rule`):

```text
N = K / R + 12 + padding,   padding 0..=4,   K the largest value keeping K / R whole
```

For PSK/QAM levels the coded bits fill the data cells exactly:
`data_cells = N / bits per symbol` (`data_cells`). That is every cell minus the
pilots minus one reference cell per carrier; 16QAM/32QAM frames reserve 19 more
cells per carrier (section 4.5).

OFDM subcarriers sit on contiguous FFT bins centred on bin 32 (= 1500 Hz):
2300 Hz uses bins 8..56, 2750 Hz bins 3..61, 500 Hz bins 27..37 (`ofdm_bins`).

### 3.8 FSK parameters (`FskParams`, `tests/fsk.rs`)

FSK levels hop a single tone over FFT bins `first_bin..=last_bin` of a
`samples_per_symbol`-point FFT at 3000 Hz, moving `spacing` bins per symbol
value, with `bits` coded bits per data symbol:

| Bandwidth | Level | Bits | Bins | Spacing | Tone positions |
|---|---|---|---|---|---|
| 500 | 1 | 3 | 54..74 | 2 | 11 |
| 500 | 2, 3 | 3 | 27..37 | 1 | 11 |
| 2300 | 1 | 3 | 33..96 | 2 | 32 |
| 2300 | 2 / 3 | 3 / 4 | 17..48 | 2 | 16 |
| 2300 | 4 | 4 | 9..24 | 1 | 16 |
| 2750 | 1 | 3 | 24..103 | 2 | 40 |
| 2750 | 2 / 3 | 3 / 4 | 12..51 | 2 | 20 |
| 2750 | 4 | 4 | 6..25 | 1 | 20 |

**Observation.** The published "carriers" of an FSK level are its tone
positions, `ceil((last − first + 1) / spacing)`, on all 11 FSK levels
(`tone_positions_match_the_published_carriers`). An FSK frame has 24 pilot
symbols; the others carry `bits` coded bits each and fill N exactly
(`data_symbols_fill_the_coded_bits`).

---

## 4. Transmit waveform (`src/fft.rs`, `src/ofdm.rs`, `src/turbo.rs`, `src/mapping.rs`, `src/frame.rs`, `src/fsk.rs`)

The sender side, layer by layer from math to audio. Everything here is
self-contained (no crates), tested as units, and consistent end-to-end: the
turbo encoder's output feeds the frame assembler, the frame assembler feeds
the OFDM modulator, the modulator produces 48 kHz audio. FSK levels take a
separate path from the encoder to a tone sequence (section 4.6).

### 4.1 Complex arithmetic and FFT (`src/fft.rs`)

A minimal `Complex` (f64) and an in-place radix-2 FFT sized for powers of
two. No external crates — the whole waveform layer keeps the zero-dependency
rule. `tests/fft.rs` checks it against a direct DFT for lengths 1..1024
(`matches_naive_dft`), the inverse (`inverse_restores_input`) and Parseval's
identity (`parseval`).

### 4.2 OFDM at the sound-card rate (`src/ofdm.rs`)

The structural constants of a symbol:

| Quantity | Baseband (3000 Hz) | Audio (48 000 Hz, ×16) |
|---|---|---|
| FFT length | 64 | 1024 |
| Cyclic prefix | 8 | 128 |
| Symbol length | 72 | 1152 |
| Symbol duration | 24 ms | 24 ms |

Subcarrier `k` sits at `k × 46.875 Hz` (`bin_frequency_hz` in
`src/modes.rs`): the bin spacing is the same at both rates, 3000/64 =
48000/1024. The OFDM symbol is 72 baseband samples, 41.667 baud; the other
levels use 128, 64 and 32 samples per symbol (section 3.7).

**Modulation.** One frame is `frame_symbols` OFDM symbols. Per symbol, the
cells (complex, `[symbol][carrier]`) are placed on the subcarrier bins as a
**Hermitian pair** (`c` at bin `k`, `conj(c)` at bin `N−k`, both scaled
`N/2`), the inverse FFT produces the time signal, and the real part is taken:
a unit cell becomes a unit cosine. A **cyclic prefix** is emitted by copying
the last 128 real samples of the symbol in front of it, so the complete
symbol is CP + FFT-body — `cyclic_prefix_repeats_the_symbol_tail` and
`demodulation_inverts_modulation` pin both halves, and
`energy_stays_on_the_level_bins` checks nothing leaks off the bandwidth's
bins.

**Demodulation.** Symbol-aligned audio is cut into `SYMBOL_LEN` chunks, the
prefix is skipped, the FFT is taken, and bins are read back scaled `2/N`.

**Frame pilot map** (`FrameMap::from_record`). The record's pilot mask is
stored carrier-by-carrier; converted to `[symbol][carrier]`, **mask entry
`k` describes OFDM symbol `k + 2`** (`MASK_FIRST_SYMBOL = 2`): symbol 0 is
the reference symbol and symbol 1 is never a pilot. The resulting pilot
symbols equal the record's pilot positions on every PSK/QAM level
(`real_frame_maps_match_pilot_positions_and_data_cells`). The mask's shape must
match the level's grid, and its last two entries, which would point past the
end of the frame, must be clear. The map then defines:

- `pilot_count()` — pilot cells in the frame;
- `data_cells()` = `(symbols − 1) × carriers − pilot_count()` — symbol 0 is
  the reference symbol and carries no data bits.

### 4.3 Turbo FEC (`src/turbo.rs`)

The error-correction code is a **turbo code using the 3GPP constituent
encoder** — the open point of the former open question 5, now settled:

- **Constituent RSC encoder**: feedback `1 + D² + D³`, output parity
  `1 + D + D³`, constraint length 4 (`8` states) — `rsc_step` is the state
  machine, tested directly.
- **Two encoders**: the second reads the information bits **through the
  level's interleaver**, which must be a permutation of the same length
  (`NotAPermutation` otherwise).
- **Puncturing** with period `Pd` per rate: `(1,3) → 1`, `(1,2) → 2`,
  `(2,3) → 4`, `(3,4) → 6`, `(4,5) → 8`, `(5,6) → 10`. At step `t = i mod Pd`
  the first encoder's parity is sent when `t == 0` and the second's when
  `t == Pd/2`; the systematic bit is always sent
  (`parities_sent`, `coded_len`).
- **Termination**: after the payload, the trellis is driven back to state 0
  with `termination_input` (the bit that makes the feedback vanish) over
  three steps — 12 tail bits total (`TAIL_BITS`), sent **unpunctured** as
  `sys1, p1, sys2, p2` per step.

**Padding.** The encoder's output (`coded_len`) is at most 4 bits shorter than
the level's N. It equals N exactly at rates 1/3, 1/2 and 2/3; at 4/5 and 5/6
it is 0..4 bits short, N being rounded up to a whole number of data cells
(`coded_length_fills_the_frame`). The missing bits are taken as 0
(section 4.5): the original reads bits past N as 0, but what the 0..4
padding bits hold is not shown directly; zero is the working assumption.

### 4.4 Constellations (`src/mapping.rs`)

The bit→complex maps, values taken **exactly as the protocol stores them**
(f32 originals; `table_values_are_bit_exact` in `tests/mapping.rs` compares
every point's raw bits):

- **BPSK** — points on the real axis;
- **QPSK** — points on the axes (not the 45° diagonals);
- **8PSK** — Gray-labelled;
- **16QAM** — Gray square, Gray-coded on each axis;
- **32QAM** — a cross: odd levels ±1, ±3, ±5 per axis without the four
  corners (`qam32_is_a_cross`; its labelling is not claimed to be Gray).

For BPSK, QPSK and 8PSK the table also agrees point by point with the
original's receive-side decisions, a separate part of it
(`psk_table_agrees_with_vara_demapper`).

The first bit of a group is the most significant bit of the index
(`index_msb_first`).

### 4.5 Frame assembly — differential PSK (`src/frame.rs`)

How cells get their values (`assemble`). Order of events:

1. **Channel interleave**: `interleaved[i] = coded[A[i]]`, with the coded
   stream padded with zeros when shorter than the interleaver
   (`channel_interleave`).
2. Per carrier, walk the symbols:
   - symbol `0` — the **reference symbol**, a constellation point the
     original draws at random for every carrier; the interface takes it
     explicitly, so tests and callers can pin it;
   - a **pilot cell** — repeats the previous symbol on that carrier;
   - a **data cell** — the previous symbol **multiplied** by the
     constellation point selected by the next bits: differential PSK (DPSK).
     Two consecutive symbols therefore only ever differ by the encoded
     rotation — the receiver decodes phase *changes*, not absolute phase.
3. The bit stream is consumed strictly: exactly `data_cells() × bits/symbol`
   bits on PSK levels (`FrameError::BitCount` otherwise), each chunk mapped
   MSB-first.

**QAM levels are not differential.** 16QAM/32QAM are sent coherently, with
19 extra reference cells per carrier described outside the record's pilot mask
(`real_frame_maps_match_pilot_positions_and_data_cells` checks the count).
Where those cells sit and what they hold is not yet known, so `assemble`
rejects QAM levels with `FrameError::CoherentQam` rather than guess
(open question 7).

### 4.6 FSK tone sequence (`src/fsk.rs`, `tests/fsk.rs`)

FSK levels are **frequency-hopped MFSK**: one tone per symbol, no reference
symbol. For frame symbol `t` (`tone_bins`):

- a **data symbol** takes the next `bits` coded bits (first bit most
  significant) as `v` and sends `d = gray_decode(v)`, the inverse Gray code —
  neighbouring tones carry labels one bit apart (`symbol_values_are_inverse_gray`);
- a **pilot symbol** (flagged in the record's pairs) sends the record's `d`;
- the tone is

  ```text
  bin = first + ((map[t] + spacing × d − first) mod (last − first + 1))
  ```

  `map[t]` being the record's base bin for that symbol. The base hops across
  the whole band from symbol to symbol; the data shifts the tone from there,
  wrapping round inside the band (`tone_formula_with_wraparound`).

Bin `k` sounds at `k × 3000 / samples_per_symbol` Hz. With VARA's tables every
tone falls inside the band (`real_tables_give_tones_inside_the_band`), and the
24 flagged symbols are exactly the record's pilot list
(`real_pilots_are_flagged_where_listed`).

### 4.7 Not yet built on the sender side

- FSK audio: how the phase carries from one symbol to the next, and the
  lead-in of random symbols the original sends before the data;
- coherent QAM grid assembly;
- single-carrier PSK audio (levels at 94 baud): the pulse shape;
- the payload itself: what goes into the information bits (frame headers,
  the radio-frame CRC, the format of service frames) — the layer above FEC;
- resampling to other sound-card rates and real audio/PTT I/O.

---

## 5. Command channel (`src/control.rs`, `src/session.rs`, `src/server.rs`)

The modem is driven over TCP port 8300 by an application (PAT, Winlink
Express). Everything in this section was observed between Winlink Express and
the original modem; callsigns in the tests are placeholders.

### 5.1 Lines (`tests/control.rs`)

- ASCII lines ending in **CR only** (`\r`); LF is not a terminator
  (`line_reader_does_not_treat_lf_as_terminator`).
- Application → modem (`Command`): `WINLINK SESSION`, `CONNECT <own> <target>`,
  `ABORT`, `BW500` / `BW2300` / `BW2750`. Anything else is kept verbatim as
  `Command::Unknown`.
- Modem → application (`Event`): `OK`, `PTT ON`, `PTT OFF`, `BUSY ON`,
  `BUSY OFF`, `DISCONNECTED`, `IAMALIVE`.
- The modem sends some events **together in one TCP segment**: `OK`,
  `PTT ON`, `BUSY ON` in answer to `CONNECT`; `DISCONNECTED`, `BUSY OFF` at
  the end of a call (`grouped_events_encode_to_observed_segments`).
- A line longer than `MAX_LINE` (1024 bytes) without a terminator is an error
  (`LineTooLong`), and the server closes that connection.

### 5.2 Call without answer (`tests/session.rs`)

`Session` holds the behaviour on a caller-supplied clock, with no I/O: the
caller passes commands and fires timers, and every returned batch is one
segment. Observed behaviour it reproduces:

| Behaviour | Value |
|---|---|
| Call attempts before giving up | 15 |
| First call frame (PTT on → off) | 1.786 s |
| Later call frames | 1.431 s |
| First PTT on → second PTT on | 3.110 s |
| Listening between later frames | 1.295 s |
| Last PTT off → `DISCONNECTED` | 1.320 s |
| `IAMALIVE` while idle | every 60 s |

These five durations reproduce every event of a captured 15-attempt call to
within 38 ms (`unanswered_call_matches_capture`). `ABORT` is answered `OK` at
once, but the frame on air plays to its end before `PTT OFF`
(`abort_during_transmission_matches_capture`).

Timings that could never progress are refused when a session is created:
a zero keepalive interval (`ZeroKeepalive`) and zero attempts
(`ZeroAttempts`).

**Assumptions** (not in the captures, marked as such in the tests): `ABORT`
between two frames hangs up as after the last frame; no `IAMALIVE` during a
call; no reply to unknown commands.

### 5.3 Server and binary (`tests/server.rs`, `tests/cli.rs`)

`serve_client` runs one connection on one thread: the socket read timeout is
the time to the session's next deadline, and timers fire at their scheduled
times rather than at wake-up, so the schedule does not drift. Each batch of
events is one write. The tests run it over loopback with the clock 100 times
faster.

```sh
vara-native --bind 127.0.0.1:8300   # the default
```

The data port (8301) is not served yet: its format has not been observed.

---

## 6. Running the test suite

```sh
cargo test                # self-contained: synthetic files only
VARA_DIR=/path/to/installed/VARA cargo test   # additionally checks real files
```

When `VARA_DIR` is unset, the tests that read real table files are skipped
(`tests/dat.rs` returns early). The repository itself contains no vendor
data — parsers, not tables.

---

## 7. Open questions

1. Do radio frames use this same CRC, and if so with the same off-by-one
   scope? Decide from recordings with known payloads. `src/dat.rs` stays
   free of that assumption so the frame layer can decide on evidence.
2. ~~What is the extra block of `VARAHF500.dat` for?~~ **Settled:** it is part
   of the original's licensing, not of the modem, and is deliberately left
   undecoded.
3. ~~Are the leading permutations common-frame interleavers?~~ **Settled:**
   they are the two interleavers of level 1 (section 2.7). Still open: the
   role of the two zero words that precede the CRC in every block.
4. What are the two service frame grids (11 × 50 and 2 × 450) used for, and
   which audio frequencies do their carriers occupy?
5. ~~What lies between the pilot masks in the table files?~~ **Settled:**
   the two interleavers of each level, plus pilot positions (PSK) or the tone
   map and pilot pairs (FSK). Every word of the three blocks belongs to a
   record (section 2.7).
6. Exact symbol rates: the published baud figures are rounded; the
   3000 Hz baseband with 128/64/32 samples-per-symbol agrees with
   23.4375 / 46.875 / 93.75 / 41.67 baud. Structurally confirmed by the
   waveform layer; final confirmation against real audio (and true
   subcarrier exactness under frequency error) is pending.
7. Where are the 19 extra reference cells per carrier of the 16QAM/32QAM
   frames, and what do they hold? Until then `assemble` refuses QAM levels
   (`FrameError::CoherentQam`).
8. ~~Where does the reference symbol come from?~~ **Settled:** the original
   draws a random constellation point per carrier and frame; the receiver,
   decoding phase changes, does not need to know it.
9. FSK audio: phase continuity between symbols, and the lead-in of random
   symbols sent before the data (its length depends on the level).
10. What do the 0..4 padding bits at rates 4/5 and 5/6 hold? Zero is assumed.
11. The layer above the FEC: frame headers, the radio-frame CRC (question 1),
    callsign encoding, and the call/acknowledge frames of a session.
