# vara-native

A native, independent implementation of the VARA HF radio-data protocol for Linux.

**Status:** work in progress. The table formats, speed levels, turbo encoder,
constellations, OFDM and FSK structure and the command channel for an
unanswered call are implemented and tested; synchronisation, frame headers and
the session protocol are not yet known, so it does not talk to a real station
yet (see [Roadmap](#roadmap)).
**License:** [GPL-3.0-or-later](LICENSE). **Not affiliated with the original VARA author.**

---

## What is this?

VARA HF is a well-known data-over-radio protocol used throughout the amateur
radio world, most prominently as the transport behind [Winlink](https://www.winlink.org)
email gateways. Until now, the only available modems implementing it have been
Windows/Mac applications, commonly run on Linux through an emulation layer.

**vara-native is a from-scratch Linux implementation of the same protocol.**
Its goal: plug-and-play compatibility with the existing ecosystem —
PAT, LinBPQ, Direwolf-based stacks, custom KISS clients — with no emulation,
no GUI dependencies, and no Windows audio stack.

One day it should run on anything with an ALSA device and a TCP stack —
a Raspberry Pi at a remote field station, a headless gateway box, a tiny
single-board computer on emergency power.

## Why

Radio email — sending and receiving mail over shortwave or VHF radio when the
internet is unavailable, unreachable, or destroyed — is one of the most useful
and humbling corners of amateur radio. Sailors cross oceans with it.
Emergency coordinators depend on it when cellular networks go down. Daily
volunteers practice with it before they ever need it.

Today this only works as well as it does because of dedicated, proprietary
software. There is real value in having a **native, open, auditable
implementation** of the protocol it speaks:

- **Runs anywhere Linux runs**, including ARM and headless systems. No
  emulation layer between the modem and the sound device.
- **Integrates like a service**, not like a desktop app: a modem that comes
  up at boot, is scriptable, script-drivable, and observable.
- **Fits the ecosystem** by speaking to the ports and clients everyone
  already uses (command channel `8300`, KISS data channel `8301`).
- **Can be studied, extended, and taught from.** A protocol this important
  deserves a readable, open description and an honest reference design.

## Philosophy

This project is built on a few deliberate commitments.

### 1. Interoperability over imitation

The goal is not to look like the original — the goal is to **interoperate**
with it on the air, byte for byte, and to do so by understanding what the
protocol *does*, not by copying anything it *is*. Every behavior is described
first in plain language (see the `doc/` directory), and the code
is written against that description. The specification is the shared language;
the implementation follows from it.

### 2. Layers, honestly separated

The protocol is naturally layered, and the codebase mirrors that exactly:

| Layer | Responsibility | State |
|---|---|---|
| **Control plane** | TCP `8300` session command language (`WINLINK SESSION`, `CONNECT`, `ABORT`, `BW…`, …) | call, abort and keepalive done; other commands open |
| **Transport plane** | TCP `8301` data / KISS framing | not started (format not yet observed) |
| **Session** | Callsign hashing, bandwidth negotiation, retry cycles, connect/ACK/NACK state machine | retry cycle of an unanswered call done; the rest open |
| **PHY** | Speed levels, FEC, interleaving, pilots, modulation (FSK, BPSK/QPSK/8PSK, 16/32QAM), OFDM | transmit structure done except QAM cells; sync, preamble and headers open |
| **Audio** | ALSA capture/playback, PTT (CAT / VOX / GPIO) | not started |

Each layer is testable in isolation. The command and KISS planes can be
verified against real clients long before the DSP exists; each PHY construct
— cyclic prefix length, subcarrier spacing, per-level frame maps — is stated
as a checkable assertion that a recording either confirms or refutes.

### 3. Honest about what data is what

The modem needs a small set of fixed tables (interleavers, frame maps).
A modem implementation should not ship data it cannot explain, so this
repository ships **parsers, not tables**:

- The format of each table is documented in `doc/protocol.md`, down to
  every word of the files.
- The tables are read directly from a copy of the original software
  **that the user obtains and licenses independently**; the parser checks
  their size and checksum before use.
- The tables themselves are never committed or redistributed.

This keeps the repository fully self-describing, and gives every user a
legitimate, self-service path to a working setup. (This project does not
touch the original software's licensing and contains nothing that unlocks or
bypasses it. Registering the original software remains a matter between its
users and its author.)

### 4. Small, boring, readable

Radio code is failure code — it runs at 3 a.m. in an emergency shelter, on a
boat, on a battery. So: no framework, no cleverness, no hidden threads.
Rust with no external crates, synchronous where possible, every module
independently testable, every protocol constant named rather than magicked.
Boring is a feature: on a bad night, you want boring.

### 5. Respect upstream

This project exists only because the original author created the protocol and
kept improving it for years. That deserves explicit respect:

- It is an *independent, compatible implementation*, not a replacement claim
  or a fork of anyone's code.
- It does not circumvent licensing, does not redistribute anyone else's
  work, and says so plainly wherever someone might reasonably wonder.
- If you like what VARA does over the air, do what makes it possible —
  support the original author and register the original software when you
  want its full performance for your own station.

## Usage

What runs today — the command channel, without radio:

```sh
cargo run -- --bind 127.0.0.1:8300
# in another terminal: a call that is never answered, with the original's timing
printf 'CONNECT N0CALL N1CALL\r' | nc 127.0.0.1 8300
```

The test suite:

```sh
cargo test                                   # self-contained
VARA_DIR=/path/to/installed/VARA cargo test  # also checks the real table files
```

Planned shape once audio exists:

```text
vara-native --audio hw:1,0 --ptt cat:/dev/ttyUSB0 --bind 127.0.0.1:8300
# then connect PAT to 127.0.0.1:8300/8301 and talk to a real gateway.
```

## Roadmap

Done:

- **Table files** — loader with checksum, record by record (`src/dat.rs`).
- **Speed levels** — all 46 levels of the three bandwidths, with frame
  payload, exact symbol rates and code rates; they reproduce every published
  net rate (`src/modes.rs`).
- **Turbo encoder** — constituent code, puncturing, tail (`src/turbo.rs`).
- **Constellations and frame cells** — bit-exact constellations, channel
  interleaving, differential PSK frames (`src/mapping.rs`, `src/frame.rs`).
- **OFDM** — modulation and demodulation at 48 kHz, frame pilot maps
  (`src/fft.rs`, `src/ofdm.rs`).
- **FSK** — hopped tone sequence (`src/fsk.rs`).
- **Command channel** — line codec, unanswered-call state machine with the
  original's timing, TCP server and binary (`src/control.rs`,
  `src/session.rs`, `src/server.rs`).

Next:

1. **Coherent QAM frames** and **FSK/single-carrier audio**.
2. **Receiver** — turbo decoder, demodulators, then synchronisation.
3. **Preamble, frame headers, session protocol** — these need recordings of
   the original on the air; they are what makes real interoperability.
4. **Data port 8301 and the remaining commands**, from captures of a
   successful session.
5. **Audio + PTT**, packaging for Raspberry Pi / Debian, headless init script.

Open points are listed, with what would settle them, at the end of
`doc/protocol.md`.

Contributions welcome — especially: **audio recordings and TCP captures of
real VARA sessions** (they settle most open points), test vectors, and
documentation improvements.

## Repository map

```text
doc/protocol.md   protocol description; every observation names the test that pins it
doc/glossary.md   project and radio terms
src/              implementation, one module per layer
tests/            tests per module; real-file tests run when VARA_DIR is set
```

## Acknowledgements

- The original VARA author, EA5HVK, for years of quiet, sustained work on a
  protocol that a great many stations quietly depend on.
- The Winlink development team for the ecosystem that makes radio email real.
- The Linux amateur radio community that has kept this use case alive on
  unsupported platforms for years.

---

*vara-native is an independent project, not endorsed by or affiliated with
the original VARA software or its author, and it contains no code from it.
"VARA" is used nominatively to identify the protocol with which this project
interoperates.*
