# vara-native

A native, independent implementation of the VARA HF radio-data protocol for Linux.

**Status:** work in progress — protocol layer first, DSP layers next.
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
| **Control plane** | TCP `8300` session command language (`WINLINK SESSION`, `LISTEN`, `VERSION`, …) | first target |
| **Transport plane** | TCP `8301` KISS framing (`C0 … C0`, AX.25 addressing) | first target |
| **Session** | Callsign hashing, bandwidth negotiation, retry cycles, connect/ACK/NACK state machine | next |
| **PHY** | OFDM framing, pilot placement, interleaving, FEC, per-level modulation (FSK/QPSK/8PSK/QAM, 17 speed levels) | later |
| **Audio** | ALSA capture/playback, PTT (CAT / VOX / GPIO) | later |

Each layer is testable in isolation. The command and KISS planes can be
verified against real clients long before the DSP exists; each PHY construct
— cyclic prefix length, subcarrier spacing, per-level frame maps — is stated
as a checkable assertion that a recording either confirms or refutes.

### 3. Honest about what data is what

The modem needs a small set of fixed tables (interleavers, frame maps).
A modem implementation should not ship data it cannot explain, so this
repository ships **parsers, not tables**:

- The format of each table is documented here.
- A small script extracts the tables from a copy of the original software
  **that the user obtains and licenses independently**.
- The generated tables stay local and are never redistributed.

This keeps the repository fully self-describing, and gives every user a
legitimate, self-service path to a working setup. (Unregistered operation of
the protocol comes with reduced speed; this project honors that distinction
by design and includes no mechanism to bypass it. Full speed remains,
rightly, a matter between the user and the original author.)

### 4. Small, boring, readable

Radio code is failure code — it runs at 3 a.m. in an emergency shelter, on a
boat, on a battery. So: no framework, no cleverness, no hidden threads.
Plain C/C++ (or Rust — see roadmap), synchronous where possible, every module
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

## Usage (planned)

```text
# Prototype shape — not yet functional:
vara-native --audio hw:1,0 --ptt cat:/dev/ttyUSB0 --listen 8300,8301
# then: connect PAT (or any KISS client) to 127.0.0.1:8300/8301,
# and talk to a real gateway over the air.
```

## Roadmap

1. **Control plane skeleton** — TCP `8300` command parser/responder, good
   enough for PAT to connect, configure, and report status to it.
2. **KISS transport** — `8301` framing with unit tests against recorded
   client traffic.
3. **Session layer** — connect/negotiate/ACK state machine against on-air
   session recordings.
4. **PHY, receive-only first** — decoding a Level-1/L-2 FSK capture end
   to end, then widening through the speed levels toward data transmit.
5. **Audio + PTT**, packaging for Raspberry Pi / Debian, headless init script.

Contributions welcome — especially: recordings, test vectors, session traces,
and documentation improvements.

## Repository map

```text
doc/        protocol description: control plane, framing, PHY structure
src/        implementation, by layer
tools/      table format parsers (extract to local tables; do not commit them)
tests/      unit tests and recorded-vector fixtures
NOTES       working log: findings marked [fact] / [hypothesis] as they are checked
```

## Acknowledgements

- The original VARA author, EA5HVK, for years of quiet, sustained work on a
  protocol that a great many stations quietly depend on.
- The Winlink development team for the ecosystem that makes radio email real.
- The Linux amateur radio community that has kept this use case alive on
  unsupported platforms for years.

---

*vara-native is an independent project, not endorsed by, affiliated with, or
derived from the original VARA software. "VARA" is used nominatively to
identify the protocol with which this project interoperates.*
