#!/usr/bin/env python3
"""
Lightweight Mock ComfyUI Server for testing Sketch2Asset's Local ComfyUI Integration
without needing a 20GB ComfyUI + GPU Checkpoint installation.

Listens on http://127.0.0.1:8188 and implements:
- GET  /object_info
- POST /upload/image
- POST /prompt
- GET  /history/<prompt_id>
- GET  /view
"""

import io
import json
import struct
import uuid
import zlib
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlparse

HISTORY_STORE = {}
IMAGE_STORE = {}
UPLOADED_FILES = set()


def make_test_png(width: int, height: int, seed: int, has_sketch: bool) -> bytes:
    """Generate a valid solid-background PNG with a clean central diamond icon for testing."""
    w = max(64, min(width, 1024))
    h = max(64, min(height, 1024))
    cx, cy = w // 2, h // 2
    radius = int(min(w, h) * 0.30)

    # Pick a distinct vibrant color based on seed & whether sketch was passed
    r_fg = 60 if has_sketch else ((seed * 37) % 180 + 50)
    g_fg = 190 if has_sketch else ((seed * 73) % 180 + 50)
    b_fg = 245 if has_sketch else ((seed * 19) % 180 + 60)

    raw_rows = bytearray()
    for y in range(h):
        raw_rows.append(0)  # PNG filter type 0
        dy = abs(y - cy)
        for x in range(w):
            dx = abs(x - cx)
            dist = dx + dy
            if dist <= radius:
                # Inner crystal icon with dark border
                if dist >= radius - 6:
                    raw_rows.extend((25, 30, 45, 255))
                else:
                    raw_rows.extend((r_fg, g_fg, b_fg, 255))
            else:
                # Pure white background (#FFFFFF) so auto-matting can also be tested
                raw_rows.extend((255, 255, 255, 255))

    def png_chunk(chunk_type: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    idat = zlib.compress(bytes(raw_rows), level=6)
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", ihdr)
        + png_chunk(b"IDAT", idat)
        + png_chunk(b"IEND", b"")
    )


MOCK_OBJECT_INFO = {
    "CheckpointLoaderSimple": {
        "input": {
            "required": {
                "ckpt_name": [
                    [
                        "pixelArtXL_v1.safetensors",
                        "sd_xl_base_1.0.safetensors",
                        "anything_v5_anime.safetensors",
                        "flux1-dev-fp8.safetensors",
                    ]
                ]
            }
        }
    },
    "UNETLoader": {
        "input": {
            "required": {
                "unet_name": [["flux1-dev.safetensors", "flux1-schnell.safetensors"]],
                "weight_dtype": [["default", "fp8_e4m3fn"]],
            }
        }
    },
    "DualCLIPLoader": {
        "input": {
            "required": {
                "clip_name1": [["t5xxl_fp8_e4m3fn.safetensors", "clip_l.safetensors"]],
                "clip_name2": [["clip_l.safetensors", "t5xxl_fp8_e4m3fn.safetensors"]],
                "type": [["flux", "sd3", "sdxl"]],
            }
        }
    },
    "VAELoader": {
        "input": {
            "required": {
                "vae_name": [["ae.safetensors", "sdxl_vae.safetensors"]]
            }
        }
    },
}


class MockComfyHandler(BaseHTTPRequestHandler):
    def _send_json(self, status: int, payload: dict):
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        parsed = urlparse(self.path)
        if parsed.path == "/object_info":
            return self._send_json(200, MOCK_OBJECT_INFO)

        if parsed.path.startswith("/history/"):
            prompt_id = parsed.path.split("/history/", 1)[1].strip("/")
            if prompt_id in HISTORY_STORE:
                return self._send_json(200, {prompt_id: HISTORY_STORE[prompt_id]})
            return self._send_json(200, {})

        if parsed.path == "/view":
            qs = parse_qs(parsed.query)
            fname = qs.get("filename", [""])[0]
            data = IMAGE_STORE.get(fname)
            if not data:
                self.send_error(404, "Image not found")
                return
            self.send_response(200)
            self.send_header("Content-Type", "image/png")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return

        self._send_json(200, {"status": "Mock ComfyUI Running"})

    def do_POST(self):
        parsed = urlparse(self.path)
        length = int(self.headers.get("Content-Length", "0"))
        raw_body = self.rfile.read(length) if length > 0 else b""

        if parsed.path == "/upload/image":
            # Extract filename from multipart payload or generate one
            fname = f"uploaded_{uuid.uuid4().hex[:6]}.png"
            UPLOADED_FILES.add(fname)
            print(f"[MockComfyUI] Received uploaded image -> {fname} ({len(raw_body)} bytes)")
            return self._send_json(200, {"name": fname, "subfolder": "", "type": "input"})

        if parsed.path == "/prompt":
            payload = json.loads(raw_body.decode("utf-8"))
            prompt_graph = payload.get("prompt", {})
            prompt_id = str(uuid.uuid4())

            # Inspect injected nodes for verification logging
            pos_texts = []
            width, height = 512, 512
            seed = 42
            has_sketch = False

            for nid, node in prompt_graph.items():
                ctype = node.get("class_type", "")
                inputs = node.get("inputs", {})
                if ctype == "CLIPTextEncode" and isinstance(inputs.get("text"), str):
                    pos_texts.append(f"#{nid}: {inputs['text'][:80]}...")
                if "width" in inputs and "height" in inputs:
                    width = int(inputs.get("width", 512))
                    height = int(inputs.get("height", 512))
                if "seed" in inputs and isinstance(inputs["seed"], int):
                    seed = inputs["seed"]
                if ctype == "LoadImage":
                    has_sketch = True

            print(
                f"[MockComfyUI] Executing /prompt (id={prompt_id[:8]}): "
                f"nodes={len(prompt_graph)}, size={width}x{height}, seed={seed}, "
                f"has_sketch={has_sketch}, texts={pos_texts}"
            )

            out_fname = f"mock_comfy_{prompt_id[:8]}.png"
            IMAGE_STORE[out_fname] = make_test_png(width, height, seed, has_sketch)
            HISTORY_STORE[prompt_id] = {
                "status": {"status_str": "success", "completed": True, "messages": []},
                "outputs": {
                    "9": {
                        "images": [
                            {
                                "filename": out_fname,
                                "subfolder": "",
                                "type": "output",
                            }
                        ]
                    }
                },
            }
            return self._send_json(200, {"prompt_id": prompt_id, "number": 1, "node_errors": {}})

        self._send_json(404, {"error": "Unknown route"})


if __name__ == "__main__":
    server = HTTPServer(("127.0.0.1", 8188), MockComfyHandler)
    print("Mock ComfyUI Server listening on http://127.0.0.1:8188 ...")
    server.serve_forever()
