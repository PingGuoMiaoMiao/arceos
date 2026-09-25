from pathlib import Path
import getpass
import json
import secrets
import struct
import sys
import time
import zlib

import serial
from xmodem import XMODEM

sys.stdout.reconfigure(errors="backslashreplace")


PORT = "COM7"
BAUDRATE = 115200
LOAD_ADDRESS = 0x8020_0000
XMODEM_TRANSFER_MODE = "xmodem"
UART_RX_READY_MARKER = b"UART0_RX_IRQ_READY"
UART_RX_PASS_MARKER = b"UART0_RX_IRQ_PASS"
SDIO_ENUMERATION_PASS_MARKER = b"SDIO_ENUMERATION_PASS"
AIC8800_INITIALIZATION_PASS_MARKER = b"AIC8800_INIT_PASS"
AIC8800_COMMAND_PASS_MARKER = b"AIC8800_COMMAND_PASS"
AIC8800_FIRMWARE_PREFIX_LENGTH = 1024
AIC8800_FIRMWARE_READY_MARKER = b"READY AIC_FIRMWARE_PREFIX 1024"
AIC8800_FIRMWARE_BLOCK_PASS_MARKER = b"AIC8800_FIRMWARE_BLOCK_PASS"
AIC8800_FIRMWARE_BOOT_PASS_MARKER = b"AIC8800_FIRMWARE_BOOT_PASS"
AIC8800_STACK_PASS_MARKER = b"AIC8800_STACK_PASS"
AIC8800_RF_MAC_PASS_MARKER = b"AIC8800_RF_MAC_PASS"
AIC8800_MANAGEMENT_PASS_MARKER = b"AIC8800_MANAGEMENT_PASS"
AIC8800_ME_PASS_MARKER = b"AIC8800_ME_PASS"
AIC8800_STA_INTERFACE_PASS_MARKER = b"AIC8800_STA_INTERFACE_PASS"
AIC8800_SCAN_PASS_MARKER = b"AIC8800_SCAN_PASS"
AIC8800_EAPOL_MESSAGE_3_PASS_MARKER = b"AIC8800_EAPOL_MESSAGE_3_PASS"
AIC8800_LINK_UP_PASS_MARKER = b"AIC8800_LINK_UP_PASS"
AIC8800_DHCP_PASS_MARKER = b"AIC8800_DHCP_PASS"
WIFI_CREDENTIALS_READY_MARKER = (
    b"READY WIFI_CREDENTIALS PASSLEN_U8 SNONCE_32 CRC32_LE"
)
AIC8800_FIRMWARE_BUNDLE = (
    ("fw_patch_table_8800d80_u02.bin", 1384),
    ("fw_adid_8800d80_u02.bin", 1708),
    ("fw_patch_8800d80_u02.bin", 32700),
    ("fw_patch_8800d80_u02_ext0.bin", 16136),
    ("fmacfwbt_8800d80_h_u02.bin", 329580),
)


def retry_transfer_after_reboot(transfer_once, reenter_uboot, attempts: int) -> bool:
    if attempts <= 0:
        raise ValueError("attempts must be positive")
    for attempt in range(1, attempts + 1):
        if transfer_once():
            return True
        if attempt == attempts or not reenter_uboot():
            return False
    return False


def read_until(uart, marker: bytes, timeout: float, echo: bool = True) -> bytes:
    deadline = time.monotonic() + timeout
    pending_attribute = "_arceos_read_until_pending"
    data = bytearray(getattr(uart, pending_attribute, b""))
    setattr(uart, pending_attribute, b"")
    while time.monotonic() < deadline:
        marker_start = data.find(marker)
        if marker_start >= 0:
            marker_end = marker_start + len(marker)
            setattr(uart, pending_attribute, bytes(data[marker_end:]))
            return bytes(data[:marker_end])

        chunk = uart.read(uart.in_waiting or 1)
        if chunk:
            data.extend(chunk)
            if echo:
                print(chunk.decode("utf-8", errors="replace"), end="", flush=True)
    raise TimeoutError(f"did not receive marker {marker!r}")


def verify_uart_receive_interrupt(uart, timeout: float = 20) -> None:
    read_until(uart, UART_RX_READY_MARKER, timeout)
    uart.write(b"K")
    uart.flush()
    read_until(uart, UART_RX_PASS_MARKER, timeout)


def verify_sdio_enumeration(uart, timeout: float = 30) -> None:
    read_until(uart, SDIO_ENUMERATION_PASS_MARKER, timeout)


def verify_aic8800_initialization(uart, timeout: float = 30) -> None:
    read_until(uart, AIC8800_INITIALIZATION_PASS_MARKER, timeout)


def verify_aic8800_command(uart, timeout: float = 30) -> None:
    read_until(uart, AIC8800_COMMAND_PASS_MARKER, timeout)


def send_aic_firmware_prefix(uart, firmware: Path, timeout: float = 30) -> None:
    read_until(uart, AIC8800_FIRMWARE_READY_MARKER, timeout)
    with firmware.open("rb") as stream:
        payload = stream.read(AIC8800_FIRMWARE_PREFIX_LENGTH)
    if len(payload) != AIC8800_FIRMWARE_PREFIX_LENGTH:
        raise ValueError(
            f"AIC firmware must contain at least {AIC8800_FIRMWARE_PREFIX_LENGTH} bytes: "
            f"{firmware}"
        )
    checksum = zlib.crc32(payload)
    if uart.write(payload) != len(payload):
        raise OSError("short UART write while sending the AIC firmware prefix")
    checksum_bytes = struct.pack("<I", checksum)
    if uart.write(checksum_bytes) != len(checksum_bytes):
        raise OSError("short UART write while sending the AIC firmware CRC32")
    uart.flush()
    print(
        f"sent AIC firmware prefix ({len(payload)} bytes, crc32={checksum:08x})",
        flush=True,
    )


def verify_aic8800_firmware_block(uart, timeout: float = 30) -> None:
    read_until(uart, AIC8800_FIRMWARE_BLOCK_PASS_MARKER, timeout)


def send_aic_firmware_bundle(
    uart,
    firmware_directory: Path,
    manifest=AIC8800_FIRMWARE_BUNDLE,
    timeout: float = 60,
) -> None:
    for name, expected_length in manifest:
        firmware = firmware_directory / name
        if not firmware.is_file():
            raise FileNotFoundError(f"AIC firmware not found: {firmware}")
        actual_length = firmware.stat().st_size
        if actual_length != expected_length:
            raise ValueError(
                f"AIC firmware size mismatch for {name}: "
                f"expected {expected_length}, actual {actual_length}"
            )
        marker = f"READY AIC_FIRMWARE {name} {expected_length}".encode("ascii")
        read_until(uart, marker, timeout)
        checksum = 0
        sent_bytes = 0
        with firmware.open("rb") as stream:
            while chunk := stream.read(4096):
                checksum = zlib.crc32(chunk, checksum)
                if uart.write(chunk) != len(chunk):
                    raise OSError(f"short UART write while sending {name}")
                sent_bytes += len(chunk)
        checksum_bytes = struct.pack("<I", checksum)
        if uart.write(checksum_bytes) != len(checksum_bytes):
            raise OSError(f"short UART CRC32 write while sending {name}")
        uart.flush()
        print(
            f"sent {name} ({sent_bytes} bytes, crc32={checksum:08x})",
            flush=True,
        )


def verify_aic8800_firmware_boot(uart, timeout: float = 180) -> None:
    read_until(uart, AIC8800_FIRMWARE_BOOT_PASS_MARKER, timeout)


def verify_aic8800_stack(uart, timeout: float = 30) -> None:
    read_until(uart, AIC8800_STACK_PASS_MARKER, timeout)


def verify_aic8800_management(uart, timeout: float = 30) -> None:
    read_until(uart, AIC8800_MANAGEMENT_PASS_MARKER, timeout)


def verify_aic8800_me(uart, timeout: float = 30) -> None:
    read_until(uart, AIC8800_ME_PASS_MARKER, timeout)


def verify_aic8800_sta_interface(uart, timeout: float = 30) -> None:
    read_until(uart, AIC8800_STA_INTERFACE_PASS_MARKER, timeout)


def verify_aic8800_scan(uart, timeout: float = 60) -> None:
    read_until(uart, AIC8800_SCAN_PASS_MARKER, timeout)


def verify_aic8800_eapol_message_3(uart, timeout: float = 30) -> None:
    read_until(uart, AIC8800_EAPOL_MESSAGE_3_PASS_MARKER, timeout)


def verify_aic8800_link_up(uart, timeout: float = 30) -> None:
    read_until(uart, AIC8800_LINK_UP_PASS_MARKER, timeout)


def verify_aic8800_dhcp(uart, timeout: float = 150) -> None:
    read_until(uart, AIC8800_DHCP_PASS_MARKER, timeout)


def send_wifi_credentials(
    uart,
    passphrase_reader=getpass.getpass,
    nonce_source=secrets.token_bytes,
    timeout: float = 120,
) -> None:
    read_until(uart, WIFI_CREDENTIALS_READY_MARKER, timeout)
    passphrase_text = passphrase_reader("Wi-Fi hotspot password (hidden): ")
    passphrase = bytearray(passphrase_text.encode("utf-8"))
    passphrase_text = ""
    if not 8 <= len(passphrase) <= 63:
        passphrase[:] = b"\x00" * len(passphrase)
        raise ValueError("Wi-Fi passphrase must encode to 8 through 63 bytes")

    station_nonce = nonce_source(32)
    if len(station_nonce) != 32:
        passphrase[:] = b"\x00" * len(passphrase)
        raise ValueError("station nonce source must return exactly 32 bytes")

    body = bytearray(1 + len(passphrase) + len(station_nonce))
    body[0] = len(passphrase)
    body[1 : 1 + len(passphrase)] = passphrase
    body[1 + len(passphrase) :] = station_nonce
    try:
        wire_body = bytes(body)
        if uart.write(wire_body) != len(wire_body):
            raise OSError("short UART write while sending Wi-Fi credentials")
        checksum = struct.pack("<I", zlib.crc32(wire_body))
        if uart.write(checksum) != len(checksum):
            raise OSError("short UART CRC32 write while sending Wi-Fi credentials")
        uart.flush()
        print("Wi-Fi credentials sent through the active UART session", flush=True)
    finally:
        passphrase[:] = b"\x00" * len(passphrase)
        body[:] = b"\x00" * len(body)


def verify_aic8800_rf_and_mac(uart, timeout: float = 60) -> None:
    read_until(uart, AIC8800_RF_MAC_PASS_MARKER, timeout)


def serial_port_from_args(arguments: list[str]) -> str:
    option = next((arg for arg in arguments if arg.startswith("--port=")), None)
    if option is None:
        return PORT
    value = option.split("=", 1)[1]
    if not value:
        raise ValueError("--port requires a non-empty serial port name")
    return value


def uboot_wait_seconds_from_args(arguments: list[str]) -> int:
    option = next(
        (arg for arg in arguments if arg.startswith("--uboot-wait-seconds=")), None
    )
    if option is None:
        return 90
    value = option.split("=", 1)[1]
    try:
        seconds = int(value)
    except ValueError as error:
        raise ValueError("--uboot-wait-seconds requires a positive integer") from error
    if seconds <= 0:
        raise ValueError("--uboot-wait-seconds requires a positive integer")
    return seconds


def enter_uboot_prompt(uart, timeout: float) -> bool:
    try:
        read_until(uart, b"U-Boot 2021.10", timeout)
    except TimeoutError:
        return False
    uart.write(b" ")
    try:
        read_until(uart, b"soph#", 15)
    except TimeoutError:
        return False
    print("U-Boot prompt detected", flush=True)
    return True


def load_binary_via_fatload(
    uart, filename: str, expected_size: int, timeout: float = 120.0
) -> None:
    """Load the application from the SD card instead of streaming it over serial.

    The default path transfers the image with XMODEM in 128-byte frames, which
    needs one stop-and-wait round trip per frame, so a multi-megabyte product
    image becomes tens of thousands of frames and tens of minutes. No transfer
    larger than 118,848 bytes has ever completed on this board. U-Boot reads the
    same image from the card FAT partition in about a second, so the image is
    copied to the card and loaded with fatload instead.
    """
    command = f"fatload mmc 0 0x{LOAD_ADDRESS:08x} {filename}"
    print(f"loading {filename} from the SD card: {command}", flush=True)
    uart.write(command.encode("ascii") + b"\r")
    uart.flush()

    marker = f"{expected_size} bytes read in".encode("ascii")
    failures = (b"Unable to read file", b"Failed to load")
    deadline = time.monotonic() + timeout
    data = bytearray()
    while time.monotonic() < deadline:
        if marker in data:
            print(f"U-Boot read {expected_size} bytes from {filename}", flush=True)
            return
        if any(failure in data for failure in failures):
            raise TimeoutError(f"U-Boot could not read {filename} from the SD card")
        chunk = uart.read(uart.in_waiting or 1)
        if chunk:
            data.extend(chunk)
            print(chunk.decode("utf-8", errors="replace"), end="", flush=True)
            if len(data) > 8192:
                del data[: len(data) - 4096]
    raise TimeoutError(
        f"U-Boot did not report reading {expected_size} bytes for {filename}"
    )


def send_xmodem_binary_once(uart, binary: Path) -> bool:
    print("starting U-Boot loadx", flush=True)
    uart.write(f"loadx 0x{LOAD_ADDRESS:08x}\r".encode("ascii"))
    uart.flush()

    preamble = bytearray()
    handshake_deadline = time.monotonic() + 10
    while time.monotonic() < handshake_deadline:
        byte = uart.read(1)
        if byte == b"C":
            print("XMODEM CRC handshake detected", flush=True)
            break
        if byte:
            preamble.extend(byte)
    else:
        print(preamble.decode("utf-8", errors="replace"))
        print("U-Boot did not start the XMODEM CRC handshake", file=sys.stderr)
        return False

    print(preamble.decode("utf-8", errors="replace"), end="")
    pending = bytearray(b"C")

    def getc(size: int, timeout: float = 1):
        if pending:
            data = bytes(pending[:size])
            del pending[:size]
            return data
        uart.timeout = timeout
        data = uart.read(size)
        return data or None

    def putc(data: bytes, timeout: float = 1):
        uart.write_timeout = timeout
        return uart.write(data)

    modem = XMODEM(getc, putc, mode=XMODEM_TRANSFER_MODE)
    with binary.open("rb") as stream:
        sent = modem.send(stream, retry=32, timeout=2, quiet=False)
    if not sent:
        return False

    deadline = time.monotonic() + 5
    response = bytearray()
    while time.monotonic() < deadline:
        chunk = uart.read(uart.in_waiting or 1)
        if chunk:
            response.extend(chunk)
            deadline = time.monotonic() + 1

    print(response.decode("utf-8", errors="replace"))
    print(f"XMODEM transfer completed; source size: {binary.stat().st_size} bytes")
    return True


def main() -> int:
    try:
        serial_port = serial_port_from_args(sys.argv[1:])
        uboot_wait_seconds = uboot_wait_seconds_from_args(sys.argv[1:])
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    wait_uboot = "--wait-uboot" in sys.argv[1:]
    jump = "--go" in sys.argv[1:]
    reboot_linux = "--reboot-linux" in sys.argv[1:]
    uart_irq_probe = "--uart-irq-probe" in sys.argv[1:]
    sdio_enumeration_probe = "--sdio-enumerate-probe" in sys.argv[1:]
    aic8800_initialization_probe = "--aic8800-init-probe" in sys.argv[1:]
    aic8800_command_probe = "--aic8800-command-probe" in sys.argv[1:]
    wifi_credentials_prompt = "--wifi-credentials-prompt" in sys.argv[1:]
    aic_firmware_prefix_option = next(
        (arg for arg in sys.argv[1:] if arg.startswith("--aic-firmware-prefix=")), None
    )
    aic_firmware_directory_option = next(
        (arg for arg in sys.argv[1:] if arg.startswith("--aic-firmware-directory=")),
        None,
    )
    ssh_host_option = next(
        (arg for arg in sys.argv[1:] if arg.startswith("--ssh-host=")), None
    )
    input_rgb_option = next(
        (arg for arg in sys.argv[1:] if arg.startswith("--input-rgb=")), None
    )
    input_meta_option = next(
        (arg for arg in sys.argv[1:] if arg.startswith("--input-meta=")), None
    )
    fatload_option = next(
        (arg for arg in sys.argv[1:] if arg.startswith("--fatload=")), None
    )
    paths = [arg for arg in sys.argv[1:] if not arg.startswith("--")]
    if len(paths) != 1:
        print(
            f"usage: {Path(sys.argv[0]).name} [--wait-uboot] [--go] <binary>",
            file=sys.stderr,
        )
        return 2

    binary = Path(paths[0])
    if not binary.is_file():
        print(f"binary not found: {binary}", file=sys.stderr)
        return 2

    expected_size = binary.stat().st_size
    input_rgb = None
    input_metadata = None
    if input_rgb_option is not None:
        input_rgb = Path(input_rgb_option.split("=", 1)[1])
        if not input_rgb.is_file():
            print(f"runtime RGB input not found: {input_rgb}", file=sys.stderr)
            return 2
        if input_rgb.stat().st_size != 1_228_800:
            print(
                f"runtime RGB input must be 1228800 bytes: {input_rgb}",
                file=sys.stderr,
            )
            return 2
        if input_meta_option is None:
            print("--input-meta is required with --input-rgb", file=sys.stderr)
            return 2
        input_meta_path = Path(input_meta_option.split("=", 1)[1])
        if not input_meta_path.is_file():
            print(f"runtime RGB metadata not found: {input_meta_path}", file=sys.stderr)
            return 2
        metadata_document = json.loads(input_meta_path.read_text(encoding="utf-8"))
        source_width, source_height = metadata_document["source_size"]
        resized_width, resized_height = metadata_document["letterbox"]["resized_size"]
        pad_x, pad_y = metadata_document["letterbox"]["padding"]
        input_metadata = (
            source_width,
            source_height,
            resized_width,
            resized_height,
            pad_x,
            pad_y,
        )
        if not all(isinstance(value, int) and value >= 0 for value in input_metadata):
            print("runtime RGB metadata fields must be non-negative integers", file=sys.stderr)
            return 2
        if source_width == 0 or source_height == 0 or resized_width == 0 or resized_height == 0:
            print("runtime RGB dimensions must be non-zero", file=sys.stderr)
            return 2
        if pad_x + resized_width > 640 or pad_y + resized_height > 640:
            print("runtime RGB metadata exceeds the 640x640 model input", file=sys.stderr)
            return 2
    print(f"sending {binary} ({expected_size} bytes) to 0x{LOAD_ADDRESS:08x}")

    with serial.Serial(
        serial_port,
        BAUDRATE,
        bytesize=8,
        parity=serial.PARITY_NONE,
        stopbits=serial.STOPBITS_ONE,
        timeout=1,
        write_timeout=5,
        rtscts=False,
        dsrdtr=False,
        xonxoff=False,
    ) as uart:
        uart.reset_input_buffer()
        ssh_reboot_succeeded = False
        if ssh_host_option is not None:
            import paramiko

            ssh_host = ssh_host_option.split("=", 1)[1]
            print(f"requesting a clean reboot over SSH from {ssh_host}", flush=True)
            ssh = paramiko.SSHClient()
            ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
            try:
                ssh.connect(
                    ssh_host,
                    username="root",
                    password="root",
                    timeout=10,
                    allow_agent=False,
                    look_for_keys=False,
                )
                ssh.exec_command("reboot")
                ssh_reboot_succeeded = True
                time.sleep(1)
            except Exception as error:
                print(f"SSH reboot failed: {error}", file=sys.stderr, flush=True)
                print("Reset the board once; serial U-Boot detection is still active.", flush=True)
            finally:
                ssh.close()

        if reboot_linux:
            print("requesting a clean reboot through the Linux serial console", flush=True)
            uart.write(b"\n")
            login_deadline = time.monotonic() + 20
            login_data = bytearray()
            reboot_sent = False
            username_sent = False
            password_sent = False
            while time.monotonic() < login_deadline:
                chunk = uart.read(uart.in_waiting or 1)
                if chunk:
                    login_data.extend(chunk)
                    print(chunk.decode("utf-8", errors="replace"), end="", flush=True)
                    recent = bytes(login_data[-4096:])
                    if b"login:" in recent and not username_sent:
                        uart.write(b"root\n")
                        username_sent = True
                    elif b"Password:" in recent and not password_sent:
                        uart.write(b"root\n")
                        password_sent = True
                    elif b"# " in recent:
                        uart.write(b"reboot\n")
                        reboot_sent = True
                        break
                time.sleep(0.02)
            if not reboot_sent:
                print(
                    "Could not obtain the Linux root shell through UART; "
                    "the board was not rebooted.",
                    file=sys.stderr,
                )
                return 1

        if wait_uboot:
            if ssh_reboot_succeeded:
                print("waiting for U-Boot after the SSH reboot", flush=True)
            else:
                print("waiting for U-Boot; reset the board now", flush=True)
            if not enter_uboot_prompt(uart, uboot_wait_seconds):
                if ssh_reboot_succeeded:
                    message = (
                        "U-Boot was not detected within "
                        f"{uboot_wait_seconds} seconds after SSH reboot."
                    )
                else:
                    message = (
                        f"U-Boot was not detected within {uboot_wait_seconds} seconds. "
                        "Run the script again and press RESET after the prompt."
                    )
                print(message, file=sys.stderr)
                return 1

        if fatload_option is not None:
            fatload_name = fatload_option.split("=", 1)[1].strip()
            if not fatload_name:
                print(
                    "--fatload= needs the file name as it appears on the SD card",
                    file=sys.stderr,
                )
                return 2
            try:
                load_binary_via_fatload(uart, fatload_name, expected_size)
            except TimeoutError as error:
                print(f"SD card load failed: {error}", file=sys.stderr)
                return 1
        else:

            def transfer_once():
                return send_xmodem_binary_once(uart, binary)

            def reenter_uboot():
                print(
                    "XMODEM transfer was interrupted; waiting for the board to re-enter U-Boot.",
                    flush=True,
                )
                return enter_uboot_prompt(uart, uboot_wait_seconds)

            if not retry_transfer_after_reboot(
                transfer_once, reenter_uboot, attempts=3
            ):
                print("XMODEM transfer failed", file=sys.stderr)
                return 1
        if jump:
            uart.write(f"go 0x{LOAD_ADDRESS:08x}\r".encode("ascii"))
            if uart_irq_probe:
                try:
                    verify_uart_receive_interrupt(uart)
                except TimeoutError as error:
                    print(f"UART interrupt probe failed: {error}", file=sys.stderr)
                    return 1
            elif sdio_enumeration_probe:
                try:
                    verify_sdio_enumeration(uart)
                except TimeoutError as error:
                    print(f"SDIO enumeration probe failed: {error}", file=sys.stderr)
                    return 1
            elif aic8800_initialization_probe:
                try:
                    verify_aic8800_initialization(uart)
                except TimeoutError as error:
                    print(f"AIC8800 initialization probe failed: {error}", file=sys.stderr)
                    return 1
            elif aic8800_command_probe:
                try:
                    verify_aic8800_command(uart)
                except TimeoutError as error:
                    print(f"AIC8800 command probe failed: {error}", file=sys.stderr)
                    return 1
            elif aic_firmware_directory_option is not None:
                firmware_directory = Path(
                    aic_firmware_directory_option.split("=", 1)[1]
                )
                try:
                    send_aic_firmware_bundle(uart, firmware_directory)
                    verify_aic8800_stack(uart)
                    verify_aic8800_rf_and_mac(uart)
                    verify_aic8800_management(uart)
                    verify_aic8800_me(uart)
                    verify_aic8800_sta_interface(uart)
                    verify_aic8800_scan(uart)
                    if wifi_credentials_prompt:
                        send_wifi_credentials(uart)
                        verify_aic8800_link_up(uart)
                        verify_aic8800_dhcp(uart)
                except (OSError, TimeoutError, ValueError) as error:
                    print(f"AIC8800 firmware boot probe failed: {error}", file=sys.stderr)
                    return 1
            elif aic_firmware_prefix_option is not None:
                firmware = Path(aic_firmware_prefix_option.split("=", 1)[1])
                if not firmware.is_file():
                    print(f"AIC firmware not found: {firmware}", file=sys.stderr)
                    return 1
                try:
                    send_aic_firmware_prefix(uart, firmware)
                    verify_aic8800_firmware_block(uart)
                except (OSError, TimeoutError, ValueError) as error:
                    print(f"AIC8800 firmware block probe failed: {error}", file=sys.stderr)
                    return 1
            elif input_rgb is not None:
                try:
                    read_until(uart, b"READY RGB_U8 1228800 META_U32 6", 30)
                except TimeoutError:
                    print("ArceOS did not request the runtime RGB input", file=sys.stderr)
                    return 1
                payload = input_rgb.read_bytes()
                checksum = zlib.crc32(payload)
                uart.write(struct.pack("<6I", *input_metadata))
                print(
                    f"sending runtime RGB input ({len(payload)} bytes, "
                    f"crc32={checksum:08x}, metadata={input_metadata})",
                    flush=True,
                )
                sent_bytes = 0
                for offset in range(0, len(payload), 4096):
                    chunk = payload[offset : offset + 4096]
                    uart.write(chunk)
                    sent_bytes += len(chunk)
                    if sent_bytes % (256 * 1024) == 0 or sent_bytes == len(payload):
                        print(f"runtime RGB sent: {sent_bytes}/{len(payload)}", flush=True)
                uart.write(struct.pack("<I", checksum))
                uart.flush()
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                chunk = uart.read(uart.in_waiting or 1)
                if chunk:
                    print(chunk.decode("utf-8", errors="replace"), end="", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
