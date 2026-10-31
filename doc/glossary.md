# Glossary

Every project term in one place. Terms are grouped by area; code symbols are
named where the term has a concrete implementation (`src/`, `tests/` lines
that pin it).

---

## Project-half terms

**VARA / VARA HF** — the proprietary radio-data protocol this project
interoperates with: a modem for HF SSB voice-bandwidth radio, best known as
the transport behind Winlink email gateways. Named nominatively to identify
the protocol, not the product.

**vara-native** — this project: an independent, native Linux implementation.
Not affiliated with the original author; GPL-3.0-or-later.

**Winlink** — the radio-email system commonly carried over VARA: mail
client → modem → HF/VHF RF → a gateway station (which has internet) → real
mail servers.

**RMS gateway** — a station running Winlink server software and a modem,
accepting calls over RF and relaying to the internet.

**Protocol-compatible implementation** — our framing: the goal is byte-level
interoperability on the air and on TCP, achieved from documented behavior,
not from licensing or copying anything.

---

## Radio terms (PHY)

**HF / SSB** — high-frequency shortwave (3–30 MHz), voice-bandwidth channel
(~2.4–3 kHz) via ionospheric propagation: noisy, fading, slow but
continental ranges.

**USB dial + 1500 Hz** — how VARA sits in the audio passband: the signal
centre is 1500 Hz above the transceiver's dial frequency, in USB mode. All
single-carrier frequencies in `src/modes.rs` sum to 3000 symmetrically
around this centre — that symmetry is test-pinned
(`single_carrier_frequencies_are_centred_and_even`).

**FSK** — frequency-shift keying: data is encoded as two (or a few) tones.
Slow, extremely robust; serves as the bottom of every speed ladder
(`Modulation::Fsk`).

**PSK/QAM constellations** — phase/phase-and-amplitude encoding: BPSK (1 bit
per symbol), QPSK (2), 8PSK (3), 16QAM (4), 32QAM (5). Higher bits-per-symbol
means more data per symbol and less noise tolerance — hence the ladder grows
FSK → QPSK → … → 32QAM as the channel allows.

**Symbol, symbol rate, baud** — the unit a waveform carries (one rotation of
the constellation); its rate is the baud. VARA publishes rounded figures
(23 / 47 / 94 / 42); the exact rates (23.4375 / 46.875 / 93.75 / 41.67) are
pending recording evidence — open question 6 in `doc/protocol.md`.

**Net bps** — user data rate after coding overhead; the published "bps"
figures (18 … 8489).

**OFDM** — orthogonal frequency-division multiplexing: many narrow
**subcarriers** transmitted in parallel, each carrying one fast-modulated
symbol per **OFDM symbol frame**. Wins on HF because a fade usually hits
part of the carriers, not all.

**Subcarrier spacing** — the frequency distance between subcarriers; VARA's
is 46.875 Hz (`doc/protocol.md` 3.2, open question 6).

**Pilot** — a symbol with a known value, transmitting checkmark positions the
receiver uses to track timing/phase/amplitude. Paid in throughput, spent on
robustness.

**PTT** — push-to-talk; the modem asserts/deasserts the radio's transmit
state. On the command channel it appears as `PTT ON` / `PTT OFF` events.

**Busy channel** — the modem's sense that audio is present (another signal
or noise); reported as `BUSY ON` / `BUSY OFF`. Own TX also asserts it.

---

## Frame structure

**Frame map (grid)** — a PSK/QAM level transmits in grids of
`frame_symbols × carriers` cells; some cells are pilots.
`SpeedLevel::frame_cells()` and `SpeedLevel::pilots()` hold the derived
numbers; on FSK levels both are `None`.

**Pilot mask** — in a `.dat` table file: a run of 16-bit words that are all
`0` or `1`, length `= frame_cells()`, count of ones `= pilots()`. One mask
per PSK/QAM level, in level order; two extra "service" shapes follow. See
`doc/protocol.md` 3.4.

**Service frames** — the masks that belong to no published level: 550-cell
(`11 × 50`, 4 pilots per carrier) and 900-cell (`2 × 450`, 18 per carrier).
Presumed control frames (calls/ACKs/compatibility; open question 4).

**Interleaver** — a permutation applied to data bits before transmission, so
that a burst error smears out over time/frequency and FEC can repair it. The
leading permutations (`0..299`, `0..95`) at the head of every table file are
presumed inter-leavers for common frames (open question 3).

**FEC** — forward error correction: redundant structure added so the
receiver can correct bit errors without retransmission. The strongest open
lead in the table files (index ranges tracking coded bits per frame) — not
asserted yet; open question 5.

---

## Table files

**`.dat` table file** (`VARAHF500/2300/2750.dat`) — per-bandwidth modem
constants: exact-size-checked, CRC-protected first block plus (500 Hz only)
an unprotected extra block. Documented in `doc/protocol.md` 2; implemented
in `src/dat.rs`.

**Table block** — the first block, a sequence of **big-endian** 16-bit
words; always even-length. Held as `ModemTables::words`.

**Big-endian** — byte order where the high byte comes first (`0x0102` from
bytes `01 02`).

**CRC-16/GENIBUS** — the integrity check: poly `0x1021`, init `0xFFFF`,
xorout `0xFFFF`, unreflected; check value `0xD64E` on `123456789`. These are
parameters, not choices — pinned by `tests/crc.rs` against an external
catalogue and an independent bitwise reference.

**Off-by-one scope** — VARA's CRC loop stops one byte short: the last data
byte before the stored CRC is not covered. Reproduced deliberately
(`crc16(bytes[..n-1])`), pinned by a mutation test
(`last_data_byte_is_not_covered_by_crc`).

**Extra block** — the 500-Hz-only tail (547 377 bytes, no checksum, purpose
unknown; open question 2). Held raw as `ModemTables::extra`.

---

## Command channel (TCP 8300, `src/control.rs`, `src/session.rs`, `src/server.rs`)

**Control plane / command channel** — the text-over-TCP interface: an
application (PAT, a logbook, a bot) drives the modem with ASCII lines
terminated by **CR** (`\r`, no LF).

**Command** (`Command`) — application → modem line: `WINLINK SESSION`,
`CONNECT <cs> <cs>`, `ABORT`, `BW500`/`BW2300`/`BW2750`. Unmodelled lines are
kept verbatim as `Command::Unknown`.

**Event** (`Event`) — modem → application line: `OK`, `PTT ON`/`PTT OFF`,
`BUSY ON`/`BUSY OFF`, `IAMALIVE`, `DISCONNECTED`; batched so one batch goes
out as one TCP segment (observed behavior).

**`MAX_LINE` / `LineTooLong`** — a command line longer than 1024 bytes
without a terminator is rejected instead of buffering unboundedly
(regression-tested).

**Session state machine** (`session.rs`) — behaviour with **no I/O and no
threads**: the caller feeds commands (`on_command`) and timers
(`on_timer`) on a caller-supplied clock; every returned batch is one segment.
This is what makes the timing tests deterministic and socket tests fast.

**Timings** — fitted call schedule: first frame 1.786 s, later frames
1.431 s, listen window 1.295 s, period 3.110 s → 2.73 s, 15 attempts then
hang-up, 60 s `IAMALIVE`. Verified against the captured traffic to within
38 ms (`unanswered_call_matches_capture`, `abort_during_transmission_…`).

**Attempt / call frame** — one PTT-on cycle broadcasting a call; the state
machine runs up to `attempts` of them before `DISCONNECTED`.

**Keepalive (`IAMALIVE`)** — a modem→app line every 60.0 s when idle;
proved by captures. Contributes the other half of why the real modem "feels
alive" from the client's side.

**First frame / hang-up gap** — the first call frame is longer (+0.36 s,
presumably preamble/AGC time) and after the last PTT OFF comes a ~1.3 s gap
before `DISCONNECTED`.

---

## Testing vocabulary

**TDD** — tests first, then code; the project keeps the discipline visible
in history (`Add tests … before the code exists` → `Implement …, all tests
green`).

**Mutation test** — instead of annotating expected behavior, damage the
input and assert the expected outcome (e.g. flip the last table byte →
must still parse; flip a covered byte → must fail).

**Cross-reference (bitwise reference)** — two implementations of the same
algorithm written differently must agree on every input; catches both of
them.

**Check value** — the canonical test vector for an algorithm family
(`123456789 → 0xD64E` for CRC-16/GENIBUS).

**Synthetic file** — a table file built in the test exactly to the
acceptance rules (exact size, valid CRC, valid structures), so self-contained
tests run with no vendor data.

**`VARA_DIR`** — optional environment variable pointing at an installed VARA
folder; when set, the real-file tests run. When unset they skip. This is the
materialization of **"parsers, not tables"**: no vendor bytes are ever
committed.
