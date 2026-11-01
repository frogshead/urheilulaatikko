# Simulated integration tests

Runs `lock-client-firmware` under [esp32sim](https://github.com/mikroverk/esp32sim), an
instruction-level ESP32 emulator, instead of needing a real board. The emulator boots the
chip's real mask ROM and the real ESP-IDF-format bootloader, then our unmodified app — the
same boot chain a real ESP32-C3 goes through, just not on silicon.

This tests the one thing `core`'s unit tests cannot: the real GPIO interrupt handler in
`src/wiegand_capture.rs`, driven by a scripted Wiegand keypad entry, checked against a
TOTP window computed for the emulator's actual clock, with the relay output (GPIO4)
observed through a VCD waveform. `core`'s tests use fake `Clock`/`Actuator` trait objects;
this uses the real ones, run by a real GPIO peripheral model.

## One-time setup

```bash
# 1. Build the emulator (a separate, unrelated project — not vendored here), at the
#    commit CI pins (ESP32SIM_REF in .github/workflows/rust.yml's simulate_client_firmware job)
git clone https://github.com/mikroverk/esp32sim.git ~/esp32sim
cd ~/esp32sim && git checkout 04f07602dba76eb6704b8901c989c9c9c1f8b42a
cargo build --release --bin esp32sim-c3

# 2. Fetch and SHA-256-verify the ESP32-C3 mask ROM ELF (Apache-2.0, espressif/esp-rom-elfs)
mkdir -p ~/esp32sim-roms
sh tools/fetch-rom-elfs.sh ~/esp32sim-roms

# 3. espflash, to turn our ELF into a flashable image (same tool used for real hardware;
#    CI pins ESPFLASH_VERSION in the same job, for the same reason as ESP32SIM_REF below)
cargo install espflash --locked
```

## Running

```bash
cd lock-client/firmware
export ESP32SIM_BIN=~/esp32sim/target/release/esp32sim-c3
export ESP32SIM_ROM_DIR=~/esp32sim-roms
python3 sim/run_sim_tests.py
```

Builds the firmware in release mode, computes the TOTP code for the compiled-in `SECRET`
(read straight out of `src/main.rs`, so it can't drift from what's actually flashed) at a
time just after boot, and runs two scenarios:

- **Correct code**: the relay (GPIO4) must go high within the emulated run, stay high for
  exactly `UNLOCK_DURATION` (5s), then go low again.
- **Wrong code**: the firmware must print `access denied`, never `access granted`, and the
  relay must never move.

Exits 0 on success, 1 with a `FAIL:` list naming what didn't hold. Pass `--keep` to copy the
run's flash image, scripts, console output and VCD waveforms to
`/tmp/lock-client-sim-artifacts` for inspection (e.g. with GTKWave, for the VCD).

## How the Wiegand entry is scripted

`wiegand_script.py` turns a 6-digit code into an esp32sim
[action script](https://github.com/mikroverk/esp32sim/blob/main/docs/cli.md#action-scripts):
a sequence of `<seconds> gpio <pin> <0|1>` lines that pulse GPIO2 (D0, a `0` bit) or GPIO3
(D1, a `1` bit) low and back high, timed as 4-bit nibbles per keypress — mirroring
`core::wiegand::decode_frame`'s wire-format assumption. If that assumption, the pin numbers,
or the firmware's message-gap/poll-interval constants change, update the constants at the
top of `wiegand_script.py` to match.

## In CI

`.github/workflows/rust.yml`'s `simulate_client_firmware` job runs exactly this, on every
push and pull request: it checks out `mikroverk/esp32sim` at a pinned commit (`ESP32SIM_REF`
in that job), builds `esp32sim-c3`, fetches and SHA-256-verifies the mask ROM, installs a
pinned `espflash` version, and runs this script. The commit and `espflash` version are pinned
deliberately — an upstream change to either shouldn't silently change what this job tests —
so bumping either means re-running this locally first to confirm it still passes, then
updating both `ESP32SIM_REF`/`ESPFLASH_VERSION` in the workflow and this file together.
