#!/usr/bin/env python3
"""Integration tests for lock-client-firmware against the esp32sim emulator.

Boots the real firmware image (mask ROM -> ESP-IDF bootloader -> our app,
exactly as a real ESP32-C3 does) and drives its Wiegand D0/D1 pins with a
scripted keypad entry, instead of needing a real board:

  - a code matching the current TOTP window must unlock the relay (GPIO4
    goes high) for exactly UNLOCK_DURATION, then relock
  - a wrong code must be rejected, and must never touch the relay pin

This exercises the one part of lock-client that core's unit tests cannot:
the real GPIO interrupt handler in firmware/src/wiegand_capture.rs, run by
an emulated chip, not a fake clock and a fake actuator.

Requires (see sim/README.md to build these):
  ESP32SIM_BIN       path to the esp32sim-c3 binary
  ESP32SIM_ROM_DIR   directory containing esp32c3_rev3_rom.elf
  espflash on PATH   `cargo install espflash --locked`

Usage: python3 sim/run_sim_tests.py [--keep]
"""
import hashlib
import hmac
import os
import re
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import wiegand_script  # noqa: E402

FIRMWARE_DIR = Path(__file__).parent.parent
STEP_SECONDS = 30
DIGITS = 6
UNLOCK_DURATION = 5.0  # must match UNLOCK_DURATION in src/main.rs

RELAY_PIN = "s4"  # gpio4 in the VCD, see wiegand_script.py's pin note


def run(cmd, **kwargs):
    print(f"+ {' '.join(cmd)}")
    return subprocess.run(cmd, check=True, text=True, capture_output=True, **kwargs)


def extract_build_epoch(firmware_dir: Path) -> int:
    """Forces build.rs to rerun so LOCK_CLIENT_BUILD_EPOCH reflects *this*
    run's wall-clock time, then reads the value back out of cargo's
    verbose build log — the only place it's visible before being baked
    into the binary as an opaque env!() string."""
    (firmware_dir / "build.rs").touch()
    result = run(["cargo", "build", "--release", "-vv"], cwd=firmware_dir)
    match = re.search(r"LOCK_CLIENT_BUILD_EPOCH=(\d+)", result.stdout + result.stderr)
    if not match:
        sys.exit("could not find LOCK_CLIENT_BUILD_EPOCH in the build log")
    return int(match.group(1))


def extract_secret(firmware_dir: Path) -> bytes:
    """Reads the SECRET constant straight out of main.rs, so this harness
    can never drift from the value the firmware actually compiles in."""
    source = (firmware_dir / "src" / "main.rs").read_text()
    match = re.search(r'const SECRET: &\[u8\] = b"([^"]*)";', source)
    if not match:
        sys.exit("could not find `const SECRET: &[u8] = b\"...\";` in src/main.rs")
    return match.group(1).encode()


def totp(secret: bytes, unix_time: int) -> str:
    step = unix_time // STEP_SECONDS
    mac = hmac.new(secret, struct.pack(">Q", step), hashlib.sha1).digest()
    offset = mac[-1] & 0x0F
    value = (struct.unpack(">I", mac[offset : offset + 4])[0] & 0x7FFFFFFF) % 10**DIGITS
    return f"{value:0{DIGITS}d}"


def build_firmware_and_flash_image(firmware_dir: Path, out_bin: Path, elf_path: Path) -> None:
    run(
        [
            "espflash",
            "save-image",
            "--chip",
            "esp32c3",
            "--merge",
            str(elf_path),
            str(out_bin),
        ],
        cwd=firmware_dir,
    )


def parse_vcd_relay_transitions(vcd_path: Path) -> list[tuple[float, str]]:
    transitions = []
    t_ps = 0
    for line in vcd_path.read_text().splitlines():
        if line.startswith("#"):
            t_ps = int(line[1:])
        elif line.endswith(RELAY_PIN) and line[0] in "01":
            transitions.append((t_ps / 1e12, line[0]))
    return transitions


def run_emulator(
    esp32sim_bin: str,
    rom_dir: str,
    flash_image: Path,
    elf_path: Path,
    script_path: Path,
    vcd_path: Path,
    max_seconds: float,
) -> str:
    env = {**os.environ, "ESP32SIM_ROM_DIR": rom_dir}
    result = subprocess.run(
        [
            esp32sim_bin,
            "--boot",
            "rom",
            "--flash-mb",
            "4",
            "--flash-image",
            str(flash_image),
            "--elf",
            str(elf_path),
            "--console",
            "all",
            "--script",
            str(script_path),
            "--vcd",
            str(vcd_path),
            "--max-seconds",
            str(max_seconds),
        ],
        text=True,
        capture_output=True,
        env=env,
    )
    return result.stdout + result.stderr


def main() -> int:
    keep = "--keep" in sys.argv

    esp32sim_bin = os.environ.get("ESP32SIM_BIN")
    rom_dir = os.environ.get("ESP32SIM_ROM_DIR")
    if not esp32sim_bin or not rom_dir:
        print(
            "ESP32SIM_BIN and ESP32SIM_ROM_DIR must be set — see sim/README.md",
            file=sys.stderr,
        )
        return 2

    build_epoch = extract_build_epoch(FIRMWARE_DIR)
    secret = extract_secret(FIRMWARE_DIR)

    target_time = build_epoch + 1
    correct_code = totp(secret, target_time)
    # Guaranteed different from correct_code, still a valid 6-digit string.
    wrong_code = f"{(int(correct_code) + 1) % 10**DIGITS:0{DIGITS}d}"

    elf_path = (
        FIRMWARE_DIR.parent
        / "target"
        / "riscv32imc-unknown-none-elf"
        / "release"
        / "lock-client-firmware"
    )

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        flash_image = tmp / "flash.bin"
        build_firmware_and_flash_image(FIRMWARE_DIR, flash_image, elf_path)

        failures = []

        # --- Correct code: must unlock, hold for UNLOCK_DURATION, relock ---
        script_path = tmp / "correct.txt"
        entry_end = wiegand_script.write_script(
            str(script_path), correct_code, start_at=0.5, stop_at=0.5 + 1.0 + UNLOCK_DURATION + 1.0
        )
        vcd_path = tmp / "correct.vcd"
        output = run_emulator(
            esp32sim_bin, rom_dir, flash_image, elf_path, script_path, vcd_path,
            max_seconds=entry_end + UNLOCK_DURATION + 1.5,
        )

        if "access granted" not in output:
            failures.append("correct code: firmware did not print 'access granted'")
        transitions = parse_vcd_relay_transitions(vcd_path)
        if len(transitions) != 2 or [level for _, level in transitions] != ["1", "0"]:
            failures.append(f"correct code: expected relay high-then-low, got {transitions}")
        else:
            held_for = transitions[1][0] - transitions[0][0]
            if abs(held_for - UNLOCK_DURATION) > 0.05:
                failures.append(
                    f"correct code: relay held {held_for:.3f}s, expected ~{UNLOCK_DURATION}s"
                )
        print(f"[correct code {correct_code}] relay transitions: {transitions}")
        if failures:
            print(output)

        # --- Wrong code: must be rejected, must never touch the relay ---
        script_path = tmp / "wrong.txt"
        entry_end = wiegand_script.write_script(str(script_path), wrong_code, start_at=0.5, stop_at=3.0)
        vcd_path = tmp / "wrong.vcd"
        output = run_emulator(
            esp32sim_bin, rom_dir, flash_image, elf_path, script_path, vcd_path, max_seconds=3.0
        )

        if "access denied" not in output:
            failures.append("wrong code: firmware did not print 'access denied'")
        if "access granted" in output:
            failures.append("wrong code: firmware printed 'access granted'")
        transitions = parse_vcd_relay_transitions(vcd_path)
        if transitions:
            failures.append(f"wrong code: relay moved ({transitions}), it must never move")
        print(f"[wrong code {wrong_code}] relay transitions: {transitions}")
        if failures and "wrong code" in failures[-1]:
            print(output)

        if keep:
            kept = Path("/tmp/lock-client-sim-artifacts")
            if kept.exists():
                import shutil

                shutil.rmtree(kept)
            shutil.copytree(tmp, kept)
            print(f"kept artifacts in {kept}")

    if failures:
        print("\nFAIL:")
        for f in failures:
            print(f"  - {f}")
        return 1

    print("\nPASS: correct code unlocks and relocks on schedule; wrong code never touches the relay")
    return 0


if __name__ == "__main__":
    sys.exit(main())
