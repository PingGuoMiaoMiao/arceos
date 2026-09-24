import unittest
import tempfile
from pathlib import Path
import struct
import zlib

from send_arceos_xmodem import (
    XMODEM_TRANSFER_MODE,
    retry_transfer_after_reboot,
    send_wifi_credentials,
    send_aic_firmware_bundle,
    send_aic_firmware_prefix,
    serial_port_from_args,
    uboot_wait_seconds_from_args,
    verify_aic8800_command,
    verify_aic8800_dhcp,
    verify_aic8800_initialization,
    verify_aic8800_me,
    verify_aic8800_link_up,
    verify_aic8800_scan,
    verify_aic8800_eapol_message_3,
    verify_aic8800_firmware_block,
    verify_aic8800_firmware_boot,
    verify_aic8800_stack,
    verify_aic8800_sta_interface,
    verify_sdio_enumeration,
    verify_uart_receive_interrupt,
)


class XmodemTransferModeTests(unittest.TestCase):
    def test_uses_128_byte_frames_for_noisy_board_serial_links(self):
        self.assertEqual(XMODEM_TRANSFER_MODE, "xmodem")

    def test_reenters_uboot_between_failed_transfer_attempts(self):
        outcomes = iter((False, False, True))
        events = []

        transferred = retry_transfer_after_reboot(
            transfer_once=lambda: events.append("transfer") or next(outcomes),
            reenter_uboot=lambda: events.append("uboot") or True,
            attempts=3,
        )

        self.assertTrue(transferred)
        self.assertEqual(
            events,
            ["transfer", "uboot", "transfer", "uboot", "transfer"],
        )

    def test_stops_when_uboot_cannot_be_reentered(self):
        events = []

        transferred = retry_transfer_after_reboot(
            transfer_once=lambda: events.append("transfer") or False,
            reenter_uboot=lambda: events.append("uboot") or False,
            attempts=3,
        )

        self.assertFalse(transferred)
        self.assertEqual(events, ["transfer", "uboot"])


class FakeUart:
    def __init__(self):
        self._buffer = bytearray(b"UART0_RX_IRQ_READY")
        self.writes = []

    @property
    def in_waiting(self):
        return len(self._buffer)

    def read(self, size):
        data = bytes(self._buffer[:size])
        del self._buffer[:size]
        return data

    def write(self, data):
        self.writes.append(data)
        if data == b"K":
            self._buffer.extend(b"UART0_RX_IRQ_PASS count=1 last_byte=0x4b")
        return len(data)

    def flush(self):
        pass


class VerifyUartReceiveInterruptTests(unittest.TestCase):
    def test_sends_one_known_byte_after_ready_and_waits_for_pass(self):
        uart = FakeUart()

        verify_uart_receive_interrupt(uart, timeout=0.1)

        self.assertEqual(uart.writes, [b"K"])


class VerifySdioEnumerationTests(unittest.TestCase):
    def test_waits_for_the_sdio_enumeration_pass_marker(self):
        uart = FakeUart()
        uart._buffer = bytearray(b"SDIO_ENUMERATION_PASS")

        verify_sdio_enumeration(uart, timeout=0.1)

        self.assertEqual(uart.writes, [])


class VerifyAic8800InitializationTests(unittest.TestCase):
    def test_waits_for_the_aic8800_initialization_pass_marker(self):
        uart = FakeUart()
        uart._buffer = bytearray(b"AIC8800_INIT_PASS")

        verify_aic8800_initialization(uart, timeout=0.1)

        self.assertEqual(uart.writes, [])


class VerifyAic8800CommandTests(unittest.TestCase):
    def test_waits_for_the_aic8800_command_pass_marker(self):
        uart = FakeUart()
        uart._buffer = bytearray(b"AIC8800_COMMAND_PASS")

        verify_aic8800_command(uart, timeout=0.1)

        self.assertEqual(uart.writes, [])


class VerifyAic8800StackTests(unittest.TestCase):
    def test_waits_for_the_aic8800_stack_pass_marker(self):
        uart = FakeUart()
        uart._buffer = bytearray(b"AIC8800_STACK_PASS")

        verify_aic8800_stack(uart, timeout=0.1)

        self.assertEqual(uart.writes, [])


class VerifyAic8800MeTests(unittest.TestCase):
    def test_waits_for_the_aic8800_me_pass_marker(self):
        uart = FakeUart()
        uart._buffer = bytearray(b"AIC8800_ME_PASS")

        verify_aic8800_me(uart, timeout=0.1)

        self.assertEqual(uart.writes, [])


class VerifyAic8800StaInterfaceTests(unittest.TestCase):
    def test_waits_for_the_aic8800_sta_interface_pass_marker(self):
        uart = FakeUart()
        uart._buffer = bytearray(b"AIC8800_STA_INTERFACE_PASS")

        verify_aic8800_sta_interface(uart, timeout=0.1)

        self.assertEqual(uart.writes, [])


class VerifyAic8800ScanTests(unittest.TestCase):
    def test_waits_for_the_aic8800_scan_pass_marker(self):
        uart = FakeUart()
        uart._buffer = bytearray(b"AIC8800_SCAN_PASS")

        verify_aic8800_scan(uart, timeout=0.1)

        self.assertEqual(uart.writes, [])


class VerifyAic8800EapolMessage3Tests(unittest.TestCase):
    def test_waits_for_the_authenticated_message_3_pass_marker(self):
        uart = FakeUart()
        uart._buffer = bytearray(b"AIC8800_EAPOL_MESSAGE_3_PASS")

        verify_aic8800_eapol_message_3(uart, timeout=0.1)

        self.assertEqual(uart.writes, [])


class VerifyAic8800LinkUpTests(unittest.TestCase):
    def test_waits_for_message_4_and_both_key_installations(self):
        uart = FakeUart()
        uart._buffer = bytearray(
            b"AIC8800_EAPOL_MESSAGE_3_PASS\r\n"
            b"AIC8800_EAPOL_MESSAGE_4_PASS\r\n"
            b"AIC8800_KEY_INSTALL_PASS\r\n"
            b"AIC8800_LINK_UP_PASS"
        )

        verify_aic8800_link_up(uart, timeout=0.1)

        self.assertEqual(uart.writes, [])


class VerifyAic8800DhcpTests(unittest.TestCase):
    def test_waits_for_the_dhcp_bound_pass_marker(self):
        uart = FakeUart()
        uart._buffer = bytearray(
            b"AIC8800_DHCP_STARTED\r\n"
            b"AIC8800_DHCP_PASS address=192.0.2.10/24"
        )

        verify_aic8800_dhcp(uart, timeout=0.1)

        self.assertEqual(uart.writes, [])


class Aic8800FirmwareBlockTests(unittest.TestCase):
    def test_sends_exactly_one_1024_byte_prefix_and_its_crc32(self):
        uart = FakeUart()
        uart._buffer = bytearray(b"READY AIC_FIRMWARE_PREFIX 1024")
        firmware = bytes(index & 0xFF for index in range(2048))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fmacfwbt_8800d80_h_u02.bin"
            path.write_bytes(firmware)

            send_aic_firmware_prefix(uart, path)

        self.assertEqual(uart.writes[0], firmware[:1024])
        self.assertEqual(
            uart.writes[1],
            struct.pack("<I", zlib.crc32(firmware[:1024])),
        )

    def test_waits_for_the_firmware_block_pass_marker(self):
        uart = FakeUart()
        uart._buffer = bytearray(b"AIC8800_FIRMWARE_BLOCK_PASS")

        verify_aic8800_firmware_block(uart, timeout=0.1)

        self.assertEqual(uart.writes, [])


class FakeFirmwareBundleUart:
    def __init__(self, manifest):
        self.manifest = manifest
        self.file_index = 0
        self.received = bytearray()
        name, length = manifest[0]
        self._buffer = bytearray(f"READY AIC_FIRMWARE {name} {length}".encode())

    @property
    def in_waiting(self):
        return len(self._buffer)

    def read(self, size):
        data = bytes(self._buffer[:size])
        del self._buffer[:size]
        return data

    def write(self, data):
        self.received.extend(data)
        _, expected_length = self.manifest[self.file_index]
        if len(self.received) == expected_length + 4:
            self.file_index += 1
            self.received.clear()
            if self.file_index < len(self.manifest):
                name, length = self.manifest[self.file_index]
                self._buffer.extend(f"READY AIC_FIRMWARE {name} {length}".encode())
            else:
                self._buffer.extend(
                    b"AIC8800_FIRMWARE_BOOT_PASS\r\n"
                    b"AIC8800_STACK_PASS\r\n"
                    b"AIC8800_RF_MAC_PASS\r\n"
                    b"AIC8800_MANAGEMENT_PASS\r\n"
                    b"AIC8800_ME_PASS\r\n"
                    b"AIC8800_STA_INTERFACE_PASS\r\n"
                    b"AIC8800_SCAN_PASS"
                )
        return len(data)

    def flush(self):
        pass


class Aic8800FirmwareBundleTests(unittest.TestCase):
    def test_sends_every_exactly_sized_file_followed_by_its_crc32(self):
        manifest = (("one.bin", 3), ("two.bin", 5))
        uart = FakeFirmwareBundleUart(manifest)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / "one.bin").write_bytes(b"abc")
            (path / "two.bin").write_bytes(b"12345")

            send_aic_firmware_bundle(uart, path, manifest=manifest)
            verify_aic8800_firmware_boot(uart, timeout=0.1)

        self.assertEqual(uart.file_index, 2)


class SerialPortArgumentTests(unittest.TestCase):
    def test_uses_the_explicit_serial_port(self):
        self.assertEqual(serial_port_from_args(["--port=COM3"]), "COM3")

    def test_rejects_an_empty_serial_port(self):
        with self.assertRaises(ValueError):
            serial_port_from_args(["--port="])


class UbootWaitArgumentTests(unittest.TestCase):
    def test_uses_the_explicit_positive_wait(self):
        self.assertEqual(uboot_wait_seconds_from_args(["--uboot-wait-seconds=600"]), 600)

    def test_defaults_to_ninety_seconds(self):
        self.assertEqual(uboot_wait_seconds_from_args([]), 90)

    def test_rejects_zero_non_numeric_and_empty_values(self):
        for value in ("0", "abc", ""):
            with self.subTest(value=value), self.assertRaises(ValueError):
                uboot_wait_seconds_from_args([f"--uboot-wait-seconds={value}"])


class WifiCredentialTransferTests(unittest.TestCase):
    def test_preserves_a_ready_marker_received_in_the_same_chunk_as_scan_pass(self):
        uart = FakeUart()
        uart._buffer = bytearray(
            b"AIC8800_SCAN_PASS\r\n"
            b"READY WIFI_CREDENTIALS PASSLEN_U8 SNONCE_32 CRC32_LE"
        )

        verify_aic8800_scan(uart, timeout=0.1)
        send_wifi_credentials(
            uart,
            passphrase_reader=lambda _: "test-passphrase",
            nonce_source=lambda length: bytes(range(length)),
            timeout=0.1,
        )

        self.assertEqual(len(uart.writes), 2)

    def test_sends_hidden_passphrase_and_supplied_snonce_with_crc32(self):
        uart = FakeUart()
        uart._buffer = bytearray(
            b"READY WIFI_CREDENTIALS PASSLEN_U8 SNONCE_32 CRC32_LE"
        )
        station_nonce = bytes(range(32))

        send_wifi_credentials(
            uart,
            passphrase_reader=lambda _: "test-passphrase",
            nonce_source=lambda length: station_nonce[:length],
            timeout=0.1,
        )

        body = bytes([15]) + b"test-passphrase" + station_nonce
        self.assertEqual(
            uart.writes,
            [body, struct.pack("<I", zlib.crc32(body))],
        )

    def test_rejects_passphrases_outside_the_fixed_sdk_length_range(self):
        for passphrase in ("1234567", "x" * 64):
            with self.subTest(length=len(passphrase)):
                uart = FakeUart()
                uart._buffer = bytearray(
                    b"READY WIFI_CREDENTIALS PASSLEN_U8 SNONCE_32 CRC32_LE"
                )
                with self.assertRaisesRegex(ValueError, "8 through 63"):
                    send_wifi_credentials(
                        uart,
                        passphrase_reader=lambda _, value=passphrase: value,
                        nonce_source=lambda length: bytes(length),
                        timeout=0.1,
                    )
                self.assertEqual(uart.writes, [])


if __name__ == "__main__":
    unittest.main()
