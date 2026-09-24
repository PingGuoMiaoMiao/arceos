#!/usr/bin/env python3
import argparse
import json
import struct
import sys
import urllib.error
import urllib.request
import zlib
from dataclasses import dataclass
from pathlib import Path
from typing import Callable


MAGIC = b"ARIM"
VERSION = 1
HEADER_LENGTH = 40
PAYLOAD_LENGTH = 1_228_800
TOTAL_LENGTH = HEADER_LENGTH + PAYLOAD_LENGTH
CONTENT_TYPE = "application/vnd.arceos.rgb-u8"
MODEL_NAME = "mushroom_yolov5s_cv181x_int8_sym"


@dataclass(frozen=True)
class InputMetadata:
    source_width: int
    source_height: int
    resized_width: int
    resized_height: int
    pad_x: int
    pad_y: int


Transport = Callable[[str, str, bytes | None, dict[str, str]], tuple[int, dict]]


def load_metadata(path: Path) -> InputMetadata:
    document = json.loads(path.read_text(encoding="utf-8"))
    if document["output_layout"] != "RGB_CHW_UINT8":
        raise ValueError("metadata output_layout must be RGB_CHW_UINT8")
    if document["output_size"] != PAYLOAD_LENGTH:
        raise ValueError(f"metadata output_size must be {PAYLOAD_LENGTH}")
    source_width, source_height = document["source_size"]
    resized_width, resized_height = document["letterbox"]["resized_size"]
    pad_x, pad_y = document["letterbox"]["padding"]
    if document["letterbox"]["model_size"] != [640, 640]:
        raise ValueError("metadata model_size must be [640, 640]")
    if document["letterbox"]["pad_value"] != 0:
        raise ValueError("metadata pad_value must be zero")
    return InputMetadata(
        source_width=source_width,
        source_height=source_height,
        resized_width=resized_width,
        resized_height=resized_height,
        pad_x=pad_x,
        pad_y=pad_y,
    )


def build_envelope(payload: bytes, metadata: InputMetadata) -> bytes:
    if len(payload) != PAYLOAD_LENGTH:
        raise ValueError(f"RGB CHW payload must be exactly {PAYLOAD_LENGTH} bytes")
    checksum = zlib.crc32(payload)
    header = struct.pack(
        "<4sHH8I",
        MAGIC,
        VERSION,
        HEADER_LENGTH,
        metadata.source_width,
        metadata.source_height,
        metadata.resized_width,
        metadata.resized_height,
        metadata.pad_x,
        metadata.pad_y,
        PAYLOAD_LENGTH,
        checksum,
    )
    if len(header) != HEADER_LENGTH:
        raise AssertionError("PhoneImageEnvelopeV1 header length changed")
    return header + payload


def corrupted_crc_envelope(envelope: bytes) -> bytes:
    damaged = bytearray(envelope)
    checksum = struct.unpack_from("<I", damaged, 36)[0]
    struct.pack_into("<I", damaged, 36, checksum ^ 1)
    return bytes(damaged)


def urllib_transport(base_url: str, timeout: float) -> Transport:
    root = base_url.rstrip("/")

    def request(method: str, path: str, body: bytes | None, headers: dict[str, str]):
        outgoing = urllib.request.Request(
            root + path,
            data=body,
            headers=headers,
            method=method,
        )
        try:
            with urllib.request.urlopen(outgoing, timeout=timeout) as response:
                return response.status, json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as error:
            return error.code, json.loads(error.read().decode("utf-8"))

    return request


def require_health(status: int, body: dict) -> None:
    if status != 200:
        raise RuntimeError(f"health returned HTTP {status}: {body}")
    expected = {"wifi": "DhcpBound", "tpu": "Ready", "model": MODEL_NAME}
    if body != expected:
        raise RuntimeError(f"health response mismatch: expected {expected}, actual {body}")


def require_invalid_crc(status: int, body: dict) -> None:
    if status != 400:
        raise RuntimeError(f"invalid CRC returned HTTP {status}: {body}")
    code = body.get("error", {}).get("code")
    if code != "INVALID_ENVELOPE":
        raise RuntimeError(f"invalid CRC error code mismatch: {body}")


def require_inference(status: int, body: dict, metadata: InputMetadata) -> int:
    if status != 200:
        raise RuntimeError(f"inference returned HTTP {status}: {body}")
    if body.get("model") != MODEL_NAME:
        raise RuntimeError(f"inference model mismatch: {body}")
    expected_image = {"width": metadata.source_width, "height": metadata.source_height}
    if body.get("image") != expected_image:
        raise RuntimeError(f"inference image metadata mismatch: {body}")
    if not isinstance(body.get("detections"), list):
        raise RuntimeError(f"inference detections must be a list: {body}")
    timing = body.get("timing_us")
    if not isinstance(timing, dict):
        raise RuntimeError(f"inference timing_us must be an object: {body}")
    for field in ("receive", "quantize", "tpu", "postprocess", "total"):
        value = timing.get(field)
        if not isinstance(value, int) or value < 0:
            raise RuntimeError(f"inference timing field {field} is invalid: {body}")
    request_id = body.get("request_id")
    if not isinstance(request_id, int) or request_id <= 0:
        raise RuntimeError(f"inference request_id is invalid: {body}")
    return request_id


def run_gate(
    transport: Transport,
    envelope: bytes,
    metadata: InputMetadata,
    request_count: int,
) -> dict:
    if request_count < 2:
        raise ValueError("request_count must be at least two")

    status, health = transport("GET", "/health", None, {})
    require_health(status, health)

    headers = {"Content-Type": CONTENT_TYPE}
    request_ids = []
    status, first = transport("POST", "/api/infer", envelope, headers)
    request_ids.append(require_inference(status, first, metadata))

    status, invalid = transport(
        "POST", "/api/infer", corrupted_crc_envelope(envelope), headers
    )
    require_invalid_crc(status, invalid)

    responses = [first]
    while len(responses) < request_count:
        status, response = transport("POST", "/api/infer", envelope, headers)
        request_ids.append(require_inference(status, response, metadata))
        responses.append(response)

    if any(right != left + 1 for left, right in zip(request_ids, request_ids[1:])):
        raise RuntimeError(f"request IDs are not consecutive: {request_ids}")

    return {
        "health": health,
        "invalid_crc": invalid,
        "request_count": len(responses),
        "first_request_id": request_ids[0],
        "last_request_id": request_ids[-1],
        "input_crc32": f"{struct.unpack_from('<I', envelope, 36)[0]:08x}",
        "detections_per_request": [len(item["detections"]) for item in responses],
        "timing_us": [item["timing_us"] for item in responses],
    }


def parse_args(arguments: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Verify the SG2002 ArceOS phone-upload and TPU inference gate."
    )
    parser.add_argument("--url", required=True)
    parser.add_argument("--input-rgb", type=Path, required=True)
    parser.add_argument("--input-meta", type=Path, required=True)
    parser.add_argument("--requests", type=int, default=20)
    parser.add_argument("--timeout", type=float, default=120.0)
    return parser.parse_args(arguments)


def main(arguments: list[str]) -> int:
    options = parse_args(arguments)
    payload = options.input_rgb.read_bytes()
    metadata = load_metadata(options.input_meta)
    envelope = build_envelope(payload, metadata)
    report = run_gate(
        urllib_transport(options.url, options.timeout),
        envelope,
        metadata,
        options.requests,
    )
    print(json.dumps(report, ensure_ascii=False, indent=2))
    print("SG2002_STA_PRODUCT_GATE_PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
