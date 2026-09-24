import importlib.util
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SKILL = ROOT / "codex-skills" / "board-uart-capture"
SCRIPT = SKILL / "scripts" / "uart_capture.py"

# Sixteen bytes taken from a real LicheeRV Nano capture that arrived as frames
# but could not be decoded as text at 115200.
MANGLED_LINE = bytes.fromhex("43f9b3a8aa097ab0f95794a92585b1ae")
BOOT_TEXT = b"U-Boot 2021.10\r\nDRAM: 254 MiB\r\nStarting kernel ...\r\n"


def load_capture_module():
    spec = importlib.util.spec_from_file_location("uart_capture", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class BoardUartCaptureSkillTests(unittest.TestCase):
    def test_capture_classification(self):
        module = load_capture_module()
        self.assertEqual(module.classify_capture(b""), "NO_DATA")
        self.assertEqual(
            module.classify_capture(b"OpenSBI v1.0\r\nU-Boot 2021.10\r\n"),
            "TEXT",
        )
        self.assertEqual(module.classify_capture(MANGLED_LINE), "UNREADABLE")

    def test_mixed_early_noise_and_later_boot_text_is_text(self):
        module = load_capture_module()
        early_noise = MANGLED_LINE * 8
        later_text = (
            b"U-Boot 2021.10\r\n"
            b"DRAM: 254 MiB\r\n"
            b"Starting kernel ...\r\n"
            b"Welcome to Linux\r\n"
        )
        self.assertEqual(module.classify_capture(early_noise + later_text), "TEXT")

    def test_port_selection_requires_an_unambiguous_result(self):
        module = load_capture_module()
        self.assertEqual(module.select_port(["COM3"]), "COM3")
        with self.assertRaisesRegex(ValueError, "no serial port"):
            module.select_port([])
        with self.assertRaisesRegex(ValueError, "multiple serial ports"):
            module.select_port(["COM3", "COM4"])

    def test_signal_diagnosis_separates_text_from_a_mangled_line(self):
        module = load_capture_module()

        text = module.diagnose_capture(BOOT_TEXT)
        self.assertEqual(text["state"], "UART_TEXT")
        self.assertEqual(text["classification"], "TEXT")
        self.assertLess(text["msb_ratio"], module.MSB_RATIO_TEXT_LIMIT)

        mangled = module.diagnose_capture(MANGLED_LINE * 8)
        self.assertEqual(mangled["classification"], "UNREADABLE")
        self.assertEqual(mangled["state"], "NON_TEXT_SIGNAL")
        self.assertGreaterEqual(mangled["msb_ratio"], module.MSB_RATIO_TEXT_LIMIT)
        self.assertEqual(len(mangled["bit_profile"]), 8)
        self.assertEqual(mangled["distinct_bytes"], len(set(MANGLED_LINE)))
        self.assertEqual(mangled["bytes"], len(MANGLED_LINE) * 8)
        # The hint must steer away from blind baud-rate sweeps.
        self.assertIn("baud", mangled["hint"])

        self.assertEqual(module.diagnose_capture(b"")["state"], "NO_DATA")

    def test_signal_comparison_separates_one_line_from_another(self):
        module = load_capture_module()
        mangled = MANGLED_LINE * 16

        self.assertIn(
            "verdict=SAME_SIGNAL",
            module.compare_captures("first", mangled, "second", bytes(mangled)),
        )
        self.assertIn(
            "verdict=DIFFERENT_SIGNAL",
            module.compare_captures("first", mangled, "second", BOOT_TEXT * 8),
        )

    def test_diagnosis_report_exposes_stable_markers(self):
        module = load_capture_module()
        report = module.format_diagnosis(
            "probe", module.diagnose_capture(b"OpenSBI v1.0\r\n" * 8)
        )
        self.assertIn("SIGNAL_REPORT source=probe", report)
        self.assertIn("line_state=UART_TEXT", report)
        self.assertIn("bit_profile=b0=", report)
        self.assertIn("msb_ratio=", report)

    def test_skill_requires_ready_gate_and_preserves_evidence(self):
        text = (SKILL / "SKILL.md").read_text(encoding="utf-8")
        self.assertIn("name: board-uart-capture", text)
        self.assertIn("description: Use when", text)
        self.assertIn("SERIAL_READY", text)
        self.assertIn("原始日志", text)
        self.assertIn("不得猜测", text)
        self.assertIn("--diagnose-log", text)
        self.assertIn("--compare-logs", text)


if __name__ == "__main__":
    unittest.main()
