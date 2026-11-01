"""Generates an esp32sim action script that plays a Wiegand keypad entry.

Mirrors lock-client-core's wire-format assumption (core/src/wiegand.rs,
"Protocol assumptions"): each keypress is a 4-bit nibble, MSB first; D0
pulses low for a 0 bit, D1 pulses low for a 1 bit; digits 0-9 encode as
their 4-bit value, '*' (Cancel) is 1010, '#' (Enter) is 1011.

Pin numbers (D0/D1) and the timing constants below are independently
chosen from firmware/src/main.rs and firmware/src/wiegand_capture.rs — if
those pins or MESSAGE_GAP/POLL_INTERVAL_MS change, update this file too.
BIT_INTERVAL and INTER_NIBBLE_GAP only need to stay comfortably inside
("bits of one nibble arrive close together") and outside
("keypresses are separated by more than MESSAGE_GAP") those firmware
constants respectively — the exact values don't otherwise matter.
"""

D0_PIN = 2
D1_PIN = 3

PULSE_WIDTH = 0.00005  # 50us low pulse
BIT_INTERVAL = 0.002  # 2ms between successive bit starts within a nibble
INTER_NIBBLE_GAP = 0.040  # 40ms of silence between keypresses (> firmware's 25ms message-gap timeout)

ENTER = 11
CANCEL = 10


def _nibble_bits(value: int) -> list[int]:
    return [(value >> 3) & 1, (value >> 2) & 1, (value >> 1) & 1, value & 1]


def _keys_for_code(code: str) -> list[int]:
    assert len(code) == 6 and code.isdigit(), f"expected a 6-digit code, got {code!r}"
    return [int(d) for d in code] + [ENTER]


def generate(code: str, start_at: float) -> tuple[list[str], float]:
    """Returns (script_lines, end_time) for entering `code` followed by Enter,
    starting at `start_at` emulated seconds."""
    lines = []
    t = start_at
    for value in _keys_for_code(code):
        for bit in _nibble_bits(value):
            pin = D1_PIN if bit else D0_PIN
            lines.append(f"{t:.6f} gpio {pin} 0")
            lines.append(f"{t + PULSE_WIDTH:.6f} gpio {pin} 1")
            t += BIT_INTERVAL
        t += INTER_NIBBLE_GAP
    return lines, t


def write_script(path: str, code: str, start_at: float, stop_at: float) -> float:
    """Writes the script to `path`, appending a `stop` action at `stop_at`.
    Returns the emulated time the entry (including the final Enter) finishes."""
    lines, end_at = generate(code, start_at)
    with open(path, "w") as f:
        f.write("\n".join(lines))
        f.write(f"\n{stop_at:.6f} stop\n")
    return end_at


if __name__ == "__main__":
    import sys

    code = sys.argv[1]
    start_at = float(sys.argv[2]) if len(sys.argv) > 2 else 0.5
    lines, end_at = generate(code, start_at)
    print("\n".join(lines))
    print(f"# entry finishes at ~{end_at:.6f}s emulated", file=sys.stderr)
