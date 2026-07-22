#!/usr/bin/env bash
# scripts/build_all.sh
# 一键全量构建: 编译 k_K -> 内核 -> busybox -> unifont -> initramfs -> EFI/ISO
#
# 这是 release 用的入口,被 .github/workflows/release.yml 调用
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
ARTIFACT_DIR="$ROOT/artifacts"
export ARTIFACT_DIR
# v0.1.2: VERSION 从环境变量读，默认 v0.1.1 保持向后兼容
VERSION="${VERSION:-v0.1.1}"
export VERSION

echo "==========================================="
echo "  k_K chat linuxAI ${VERSION} 完整构建"
echo "==========================================="

mkdir -p "$ARTIFACT_DIR"
# 1. 编译 k_K (rust 静态二进制, 链接 musl, 不依赖 libc)
echo "[1/5] 编译 k_K 主程序..."
cd "$ROOT/k_K"
if command -v cargo >/dev/null 2>&1; then
    # 优先 musl 静态, 失败回退 gnu
    if cargo build --release --target x86_64-unknown-linux-musl 2>&1 | tail -5; then
        cp target/x86_64-unknown-linux-musl/release/kk_chat "$ARTIFACT_DIR/k_K"
    else
        cargo build --release 2>&1 | tail -5
        cp target/release/kk_chat "$ARTIFACT_DIR/k_K"
    fi
elif command -v rustc >/dev/null 2>&1; then
    # 极简模式: 直接 rustc
    rustc --edition 2021 -O \
        --extern libc=$(find / -name "liblibc-*.rlib" 2>/dev/null | head -1 || echo "libc.rlib") \
        -L /root/.rustup/toolchains/*/lib \
        -o "$ARTIFACT_DIR/k_K" src/main.rs 2>&1 | tail -3 || true
else
    echo "!!! 警告: cargo/rustc 不可用, 跳过 k_K 编译" >&2
fi

# 2. busybox
echo "[2/5] 编译 busybox..."
bash "$HERE/build_busybox.sh"

# 3. 内核
echo "[3/5] 编译 Linux 内核..."
bash "$HERE/build_kernel.sh"

# 4. unifont + initramfs
echo "[4/5] 准备字体 + initramfs..."
bash "$HERE/fetch_unifont.sh"
bash "$HERE/build_initramfs.sh"

# 5. EFI + ISO
echo "[5/5] 生成 EFI / ISO 镜像..."
bash "$HERE/build_images.sh"

echo ""
echo "==========================================="
echo "  构建完成!"
echo "==========================================="
ls -la "$ARTIFACT_DIR"/
