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
| 2750 Hz | `0xD4F0C` (872 206) | `0xD4F0C` | — |

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
  generation parameters. Their interpretation is under discussion in the
  project working notes; the loader treats the block as opaque words,
  exactly as documented here.
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

## 3. Running the test suite

```sh
cargo test                # self-contained: synthetic files only
VARA_DIR=/path/to/installed/VARA cargo test   # additionally checks real files
```

When `VARA_DIR` is unset, the tests that read real table files are skipped
(`tests/dat.rs` returns early). The repository itself contains no vendor
data — parsers, not tables.

---

## 4. Open questions

1. Do radio frames use this same CRC, and if so with the same off-by-one
   scope? Decide from recordings with known payloads. `src/dat.rs` stays
   free of that assumption so the frame layer can decide on evidence.
2. What is the extra block of `VARAHF500.dat` for? It is intact, not covered
   by any checksum, and unaccounted for; next step is recording evidence.
3. Are the leading permutations really common-frame interleavers, and what
   is the role of the two zero words that precede the CRC in every block?
