# LicheeRV Nano mushroom model assets

The two checked-in binary files are required at Rust compile time by
`axmodel_mushroom_yolov5` and the TPU examples:

| File | Size | SHA-256 |
|---|---:|---|
| `mushroom_yolov5s_program_0_dmabuf_subfunc_1.bin` | 783,488 bytes | `ca766afb9c3fdde05e47c9126f747f406cea66f4ae52dfebaf472e1f6a4cbdde` |
| `mushroom_yolov5s_weight.bin` | 7,108,528 bytes | `acf72fc0a83fc86af26a041d4244fdf388df84fd55a70f230c51208fcc2131c8` |

They were extracted from the verified CV181x INT8 model used by this port.
The normal generated ArceOS `.bin` and `.elf` files remain ignored.

Verify the assets before a release build:

```sh
sha256sum \
  examples/tpu-execute-licheerv-nano/mushroom_yolov5s_program_0_dmabuf_subfunc_1.bin \
  examples/tpu-execute-licheerv-nano/mushroom_yolov5s_weight.bin
```
