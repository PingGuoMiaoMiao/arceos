#!/usr/bin/env python3
"""List serial ports, capture a read-only UART log, and classify the evidence."""

from __future__ import annotations

import argparse
import signal
import sys
import time
from pathlib import Path
from typing import Iterable

import serial
from serial.tools import list_ports


def classify_capture(data: bytes) -> str:
    if not data:
        return "NO_DATA"

    text = data.decode("utf-8", errors="replace")
    replacement_ratio = text.count("\ufffd") / max(len(text), 1)
    readable = sum(char.isprintable() or char in "\r\n\t" for char in text)
    readable_ratio = readable / max(len(text), 1)
    if replacement_ratio <= 0.02 and readable_ratio >= 0.85:
        return "TEXT"
    return "UNREADABLE"


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

    result = classify_capture(bytes(captured))
    print(
        f"\nCAPTURE_RESULT classification={result} bytes={len(captured)} log={log_path}",
        flush=True,
    )
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--list-ports", action="store_true")
    parser.add_argument("--port")
    parser.add_argument("--baud", type=int, default=115200)
    parser.add_argument("--log", type=Path)
    parser.add_argument("--duration", type=float, default=0.0)
    parser.add_argument("--analyze-log", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.list_ports:
        return print_ports()
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
