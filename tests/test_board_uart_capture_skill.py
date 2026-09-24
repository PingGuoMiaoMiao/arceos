import importlib.util
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SKILL = ROOT / "codex-skills" / "board-uart-capture"
SCRIPT = SKILL / "scripts" / "uart_capture.py"


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
        self.assertEqual(
            module.classify_capture(bytes.fromhex("43f9b3a8aa097ab0f95794a92585b1ae")),
            "UNREADABLE",
        )

    def test_mixed_early_noise_and_later_boot_text_is_text(self):
        module = load_capture_module()
        early_noise = bytes.fromhex("43f9b3a8aa097ab0f95794a92585b1ae") * 8
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

    def test_skill_requires_ready_gate_and_preserves_evidence(self):
        text = (SKILL / "SKILL.md").read_text(encoding="utf-8")
        self.assertIn("name: board-uart-capture", text)
        self.assertIn("description: Use when", text)
        self.assertIn("SERIAL_READY", text)
        self.assertIn("原始日志", text)
        self.assertIn("不得猜测", text)


if __name__ == "__main__":
    unittest.main()
