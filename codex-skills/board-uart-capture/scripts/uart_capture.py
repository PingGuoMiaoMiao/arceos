#!/usr/bin/env python3
"""List serial ports, capture a read-only UART log, and classify the evidence."""

from __future__ import annotations

import argparse
import math
import signal
import sys
import time
from pathlib import Path
from typing import Iterable

import serial
from serial.tools import list_ports


def is_text_byte(byte: int) -> bool:
    return byte in (9, 10, 13) or 32 <= byte <= 126


def ascii_run_max(data: bytes) -> int:
    longest = 0
    current = 0
    for byte in data:
        if is_text_byte(byte):
            current += 1
            longest = max(longest, current)
        else:
            current = 0
    return longest


def printable_ratio(data: bytes) -> float:
    if not data:
        return 0.0
    return sum(1 for byte in data if is_text_byte(byte)) / len(data)


def classify_capture(data: bytes) -> str:
    if not data:
        return "NO_DATA"

    if ascii_run_max(data) >= 64:
        return "TEXT"

    text = data.decode("utf-8", errors="replace")
    replacement_ratio = text.count("\ufffd") / max(len(text), 1)
    readable = sum(char.isprintable() or char in "\r\n\t" for char in text)
    readable_ratio = readable / max(len(text), 1)
    if replacement_ratio <= 0.02 and readable_ratio >= 0.85:
        return "TEXT"
    return "UNREADABLE"


def bit_profile(data: bytes) -> list[float]:
    """Fraction of frames whose bit N is set, for N = 0..7.

    A real UART text stream keeps bits 0..6 busy while bit 7 (the ASCII MSB)
    stays at zero.  A wrong logic level, a framing artefact or an interfering
    signal mangles all eight positions, which is exactly what makes this vector
    a usable fingerprint instead of guessing baud rates.
    """
    total = len(data)
    if total == 0:
        return [0.0] * 8
    return [sum(1 for byte in data if (byte >> bit) & 1) / total for bit in range(8)]


def byte_entropy_bits(data: bytes) -> float:
    total = len(data)
    if total == 0:
        return 0.0
    counts = [0] * 256
    for byte in data:
        counts[byte] += 1
    entropy = 0.0
    for count in counts:
        if count:
            probability = count / total
            entropy -= probability * math.log2(probability)
    return entropy


# ASCII text never sets bit 7, so any measurable MSB traffic rules text out.
MSB_RATIO_TEXT_LIMIT = 0.02


def diagnose_capture(data: bytes) -> dict[str, object]:
    profile = bit_profile(data)
    msb_ratio = profile[7]
    classification = classify_capture(data)

    if not data:
        state = "NO_DATA"
        hint = (
            "no bytes arrived; the line stayed idle. Check power, GND, TX/RX "
            "and the reset action."
        )
    elif classification == "TEXT":
        state = "UART_TEXT"
        hint = "readable UART text; identify the boot stage from the log itself."
    else:
        state = "NON_TEXT_SIGNAL"
        hint = (
            f"frames arrived but they are not UART text (msb_ratio={msb_ratio:.3f}"
            f" >= {MSB_RATIO_TEXT_LIMIT}). Retrying other baud rates cannot turn"
            " this into text: verify GND, TX/RX and logic level, then confirm that"
            " the board actually reaches a stage which prints."
        )

    return {
        "bytes": len(data),
        "classification": classification,
        "state": state,
        "printable_ratio": printable_ratio(data),
        "msb_ratio": msb_ratio,
        "ascii_run_max": ascii_run_max(data),
        "distinct_bytes": len(set(data)),
        "entropy_bits": byte_entropy_bits(data),
        "bit_profile": profile,
        "hint": hint,
    }


def format_diagnosis(label: str, report: dict[str, object]) -> str:
    profile = report["bit_profile"]
    assert isinstance(profile, list)
    profile_text = " ".join(
        f"b{index}={value:.3f}" for index, value in enumerate(profile)
    )
    return "\n".join(
        [
            f"SIGNAL_REPORT source={label} bytes={report['bytes']}",
            f"  classification={report['classification']}",
            f"  line_state={report['state']}",
            f"  printable_ratio={report['printable_ratio']:.3f}",
            f"  msb_ratio={report['msb_ratio']:.3f}",
            f"  ascii_run_max={report['ascii_run_max']}",
            f"  distinct_bytes={report['distinct_bytes']}",
            f"  entropy_bits={report['entropy_bits']:.3f}",
            f"  bit_profile={profile_text}",
            f"  hint={report['hint']}",
        ]
    )


# Heuristic thresholds for the bit-profile distance.  Two halves of one and the
# same signal stay below 0.05; readable text versus a mangled stream exceeds 0.5.
SAME_SIGNAL_DISTANCE = 0.10
SIMILAR_SIGNAL_DISTANCE = 0.30


def compare_captures(
    first_label: str, first: bytes, second_label: str, second: bytes
) -> str:
    first_profile = bit_profile(first)
    second_profile = bit_profile(second)
    distance = sum(abs(a - b) for a, b in zip(first_profile, second_profile))
    if distance <= SAME_SIGNAL_DISTANCE:
        verdict = "SAME_SIGNAL"
    elif distance <= SIMILAR_SIGNAL_DISTANCE:
        verdict = "SIMILAR_SIGNAL"
    else:
        verdict = "DIFFERENT_SIGNAL"
    return (
        f"SIGNAL_COMPARISON verdict={verdict} distance={distance:.3f} "
        f"first={first_label} second={second_label}"
    )


def select_port(port_names: Iterable[str]) -> str:
    names = list(port_names)
    if not names:
        raise ValueError("no serial port was found")
    if len(names) > 1:
        raise ValueError("multiple serial ports were found; specify --port exactly")
    return names[0]


def available_ports() -> list[tuple[str, str, str]]:
    return [(item.device, item.description, item.hwid) for item in list_ports.comports()]


def print_ports() -> int:
    ports = available_ports()
    if not ports:
        print("NO_SERIAL_PORTS")
        return 1
    for device, description, hardware_id in ports:
        print(f"PORT device={device} description={description} hwid={hardware_id}")
    return 0


def capture(port_name: str, baud: int, log_path: Path, duration: float) -> int:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    stop = False

    def request_stop(_signum: int, _frame: object) -> None:
        nonlocal stop
        stop = True

    signal.signal(signal.SIGINT, request_stop)
    started = time.monotonic()
    captured = bytearray()

    try:
        with serial.Serial(
            port=port_name,
            baudrate=baud,
            bytesize=serial.EIGHTBITS,
            parity=serial.PARITY_NONE,
            stopbits=serial.STOPBITS_ONE,
            timeout=0.2,
            write_timeout=2,
        ) as uart, log_path.open("xb") as log_file:
            print(
                f"SERIAL_READY port={port_name} baud={baud} log={log_path}",
                flush=True,
            )
            while not stop:
                if duration > 0 and time.monotonic() - started >= duration:
                    break
                chunk = uart.read(4096)
                if not chunk:
                    continue
                captured.extend(chunk)
                log_file.write(chunk)
                log_file.flush()
                sys.stdout.buffer.write(chunk)
                sys.stdout.buffer.flush()
    except FileExistsError:
        print(f"LOG_EXISTS path={log_path}", file=sys.stderr)
        return 2
    except serial.SerialException as exc:
        print(f"SERIAL_ERROR detail={exc}", file=sys.stderr)
        return 3

    raw = bytes(captured)
    result = classify_capture(raw)
    print(
        f"\nCAPTURE_RESULT classification={result} bytes={len(captured)} log={log_path}",
        flush=True,
    )
    print(format_diagnosis(str(log_path), diagnose_capture(raw)), flush=True)
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--list-ports", action="store_true")
    parser.add_argument("--port")
    parser.add_argument("--baud", type=int, default=115200)
    parser.add_argument("--log", type=Path)
    parser.add_argument("--duration", type=float, default=0.0)
    parser.add_argument("--analyze-log", type=Path)
    parser.add_argument("--diagnose-log", type=Path)
    parser.add_argument(
        "--compare-logs",
        nargs=2,
        type=Path,
        metavar=("FIRST", "SECOND"),
        help="compare the bit-profile fingerprints of two existing raw logs",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.list_ports:
        return print_ports()
    if args.compare_logs is not None:
        first_path, second_path = args.compare_logs
        first = first_path.read_bytes()
        second = second_path.read_bytes()
        print(format_diagnosis(str(first_path), diagnose_capture(first)))
        print(format_diagnosis(str(second_path), diagnose_capture(second)))
        print(compare_captures(str(first_path), first, str(second_path), second))
        return 0
    if args.diagnose_log is not None:
        data = args.diagnose_log.read_bytes()
        print(format_diagnosis(str(args.diagnose_log), diagnose_capture(data)))
        return 0
    if args.analyze_log is not None:
        data = args.analyze_log.read_bytes()
        print(
            f"CAPTURE_RESULT classification={classify_capture(data)} "
            f"bytes={len(data)} log={args.analyze_log}"
        )
        return 0

    if args.log is None:
        print("--log is required when capturing", file=sys.stderr)
        return 2

    port_name = args.port
    if port_name is None:
        try:
            port_name = select_port(device for device, _, _ in available_ports())
        except ValueError as exc:
            print(f"PORT_SELECTION_ERROR detail={exc}", file=sys.stderr)
            print_ports()
            return 2
    return capture(port_name, args.baud, args.log, args.duration)


if __name__ == "__main__":
    raise SystemExit(main())
