import json
import struct
import tempfile
import unittest
import zlib
from pathlib import Path

from verify_mushroom_web import (
    CONTENT_TYPE,
    HEADER_LENGTH,
    MODEL_NAME,
    PAYLOAD_LENGTH,
    TOTAL_LENGTH,
    InputMetadata,
    build_envelope,
    load_metadata,
    run_gate,
)


class EnvelopeTests(unittest.TestCase):
    def test_builds_the_exact_phone_image_envelope(self):
        payload = bytes((index & 0xFF) for index in range(PAYLOAD_LENGTH))
        metadata = InputMetadata(263, 191, 640, 464, 0, 88)

        envelope = build_envelope(payload, metadata)

        self.assertEqual(len(envelope), TOTAL_LENGTH)
        self.assertEqual(envelope[:4], b"ARIM")
        self.assertEqual(struct.unpack_from("<HH", envelope, 4), (1, HEADER_LENGTH))
        self.assertEqual(
            struct.unpack_from("<8I", envelope, 8),
            (263, 191, 640, 464, 0, 88, PAYLOAD_LENGTH, zlib.crc32(payload)),
        )
        self.assertEqual(envelope[HEADER_LENGTH:], payload)

    def test_loads_the_existing_preprocessor_metadata_shape(self):
        document = {
            "source_size": [263, 191],
            "output_layout": "RGB_CHW_UINT8",
            "output_size": PAYLOAD_LENGTH,
            "letterbox": {
                "model_size": [640, 640],
                "resized_size": [640, 464],
                "padding": [0, 88],
                "pad_value": 0,
            },
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "input.json"
            path.write_text(json.dumps(document), encoding="utf-8")
            self.assertEqual(
                load_metadata(path), InputMetadata(263, 191, 640, 464, 0, 88)
            )


class GateTests(unittest.TestCase):
    def test_runs_health_valid_invalid_and_twenty_request_sequence(self):
        metadata = InputMetadata(263, 191, 640, 464, 0, 88)
        payload = bytes(PAYLOAD_LENGTH)
        envelope = build_envelope(payload, metadata)
        calls = []
        next_request_id = 9

        def transport(method, path, body, headers):
            nonlocal next_request_id
            calls.append((method, path, body, headers))
            if path == "/health":
                return 200, {
                    "wifi": "DhcpBound",
                    "tpu": "Ready",
                    "model": MODEL_NAME,
                }
            crc = struct.unpack_from("<I", body, 36)[0]
            if crc != zlib.crc32(body[HEADER_LENGTH:]):
                return 400, {
                    "error": {
                        "code": "INVALID_ENVELOPE",
                        "message": "PhoneImageEnvelopeV1 validation failed",
                    }
                }
            response = {
                "request_id": next_request_id,
                "model": MODEL_NAME,
                "image": {"width": 263, "height": 191},
                "detections": [],
                "timing_us": {
                    "receive": 1,
                    "quantize": 2,
                    "tpu": 3,
                    "postprocess": 4,
                    "total": 10,
                },
            }
            next_request_id += 1
            return 200, response

        report = run_gate(transport, envelope, metadata, 20)

        self.assertEqual(len(calls), 22)
        self.assertEqual(calls[0][:2], ("GET", "/health"))
        self.assertEqual(calls[1][:2], ("POST", "/api/infer"))
        self.assertEqual(calls[1][3], {"Content-Type": CONTENT_TYPE})
        self.assertNotEqual(calls[2][2], envelope)
        self.assertEqual(report["request_count"], 20)
        self.assertEqual(report["first_request_id"], 9)
        self.assertEqual(report["last_request_id"], 28)
        self.assertEqual(report["input_crc32"], f"{zlib.crc32(payload):08x}")


if __name__ == "__main__":
    unittest.main()
