#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$repo_root"

cargo test \
    -p axdriver_sg2002_sdio \
    -p axdriver_aic8800 \
    -p axmodel_mushroom_yolov5 \
    -p arceos-mushroom-web-licheerv-nano

platform=axplat-riscv64-licheerv-nano
examples=(
    aic8800-chip-id-licheerv-nano
    aic8800-firmware-block-licheerv-nano
    aic8800-firmware-boot-licheerv-nano
    aic8800-init-licheerv-nano
    helloworld-licheerv-nano
    plic-uart-licheerv-nano
    sdio-enumerate-licheerv-nano
    tpu-dmabuf-licheerv-nano
    tpu-execute-licheerv-nano
    tpu-init-licheerv-nano
    tpu-memory-licheerv-nano
    tpu-mmio-licheerv-nano
)

for example in "${examples[@]}"; do
    make A="examples/$example" MYPLAT="$platform" defconfig
    make A="examples/$example" MYPLAT="$platform" build
done

make A=examples/mushroom-web-licheerv-nano MYPLAT="$platform" defconfig
make A=examples/mushroom-web-licheerv-nano MYPLAT="$platform" APP_FEATURES=hardware build

make ARCH=riscv64 A=examples/helloworld defconfig
make ARCH=riscv64 A=examples/helloworld build
make ARCH=riscv64 A=examples/httpserver defconfig
make ARCH=riscv64 A=examples/httpserver build

cargo fmt --all -- --check
git diff --check

echo 'SG2002 STA product regression PASS'
