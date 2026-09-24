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


def good_inference_body(metadata, request_id):
    return {
        "request_id": request_id,
        "model": MODEL_NAME,
        "image": {"width": metadata.source_width, "height": metadata.source_height},
        "detections": [],
        "timing_us": {
            "receive": 1,
            "quantize": 2,
            "tpu": 3,
            "postprocess": 4,
            "total": 10,
        },
    }


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


class GateRejectionTests(unittest.TestCase):
    """A gate that cannot fail proves nothing, so every guard needs a negative test.

    Each of these would otherwise let a board that never implemented the contract
    print SG2002_STA_PRODUCT_GATE_PASS.
    """

    def setUp(self):
        self.metadata = InputMetadata(263, 191, 640, 464, 0, 88)
        self.payload = bytes(PAYLOAD_LENGTH)
        self.envelope = build_envelope(self.payload, self.metadata)
        self.ids = iter(range(1, 500))

    def transport(
        self,
        *,
        health=None,
        health_status=200,
        invalid_crc_status=400,
        invalid_crc_code="INVALID_ENVELOPE",
        ids=None,
        mutate=None,
    ):
        def build(method, path, body, headers):
            if path == "/health":
                return health_status, (
                    health
                    if health is not None
                    else {"wifi": "DhcpBound", "tpu": "Ready", "model": MODEL_NAME}
                )
            incoming_crc = struct.unpack_from("<I", body, 36)[0]
            if incoming_crc != zlib.crc32(body[HEADER_LENGTH:]):
                return invalid_crc_status, {
                    "error": {"code": invalid_crc_code, "message": "rejected"}
                }
            response = good_inference_body(self.metadata, next(ids if ids else self.ids))
            if mutate is not None:
                mutate(response)
            return 200, response

        return build

    def gate(self, **kwargs):
        return run_gate(self.transport(**kwargs), self.envelope, self.metadata, 3)

    def test_accepts_a_fully_correct_board(self):
        # Anchors every negative test below: the same factory must pass when the
        # board really honours the contract.
        report = self.gate()
        self.assertEqual(report["request_count"], 3)
        self.assertEqual(report["first_request_id"], 1)
        self.assertEqual(report["last_request_id"], 3)

    def test_rejects_a_health_response_with_the_wrong_model(self):
        with self.assertRaisesRegex(RuntimeError, "health response mismatch"):
            self.gate(
                health={"wifi": "DhcpBound", "tpu": "Ready", "model": "someone_else"}
            )

    def test_rejects_a_health_response_that_is_not_ok(self):
        with self.assertRaisesRegex(RuntimeError, "health returned HTTP 503"):
            self.gate(health_status=503)

    def test_rejects_a_board_that_accepts_a_corrupted_checksum(self):
        with self.assertRaisesRegex(RuntimeError, "invalid CRC returned HTTP 200"):
            self.gate(invalid_crc_status=200, invalid_crc_code="OK")

    def test_rejects_the_wrong_error_code_for_a_corrupted_checksum(self):
        with self.assertRaisesRegex(RuntimeError, "error code mismatch"):
            self.gate(invalid_crc_code="SOMETHING_ELSE")

    def test_rejects_non_consecutive_request_ids(self):
        with self.assertRaisesRegex(RuntimeError, "not consecutive"):
            self.gate(ids=iter([1, 5, 6]))

    def test_rejects_an_inference_response_with_the_wrong_model(self):
        with self.assertRaisesRegex(RuntimeError, "inference model mismatch"):
            self.gate(mutate=lambda body: body.update(model="someone_else"))

    def test_rejects_an_inference_response_with_the_wrong_image_shape(self):
        with self.assertRaisesRegex(RuntimeError, "image metadata mismatch"):
            self.gate(
                mutate=lambda body: body.update(image={"width": 1, "height": 1})
            )

    def test_rejects_an_inference_response_without_timing(self):
        with self.assertRaisesRegex(RuntimeError, "timing_us must be an object"):
            self.gate(mutate=lambda body: body.update(timing_us=None))

    def test_rejects_an_inference_response_with_a_negative_timing_field(self):
        def damage(body):
            body["timing_us"]["tpu"] = -1

        with self.assertRaisesRegex(RuntimeError, "timing field tpu is invalid"):
            self.gate(mutate=damage)

    def test_rejects_an_inference_response_with_an_invalid_request_id(self):
        with self.assertRaisesRegex(RuntimeError, "request_id is invalid"):
            self.gate(mutate=lambda body: body.update(request_id=0))

    def test_rejects_an_inference_response_whose_detections_are_not_a_list(self):
        with self.assertRaisesRegex(RuntimeError, "detections must be a list"):
            self.gate(mutate=lambda body: body.update(detections={}))

    def test_requires_at_least_two_requests(self):
        with self.assertRaisesRegex(ValueError, "at least two"):
            run_gate(self.transport(), self.envelope, self.metadata, 1)


if __name__ == "__main__":
    unittest.main()
