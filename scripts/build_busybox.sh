#!/usr/bin/env bash
# scripts/build_busybox.sh
# 编译静态链接的 busybox,作为 initramfs 的 /sbin/init
#
# 目标: 静态二进制,wget 支持 https (用于 AI 对话),ash,基础工具
# 输出: artifacts/busybox (~1.5 MB)
#
# 需要 host: gcc, make, perl
# Ubuntu/Debian: sudo apt install build-essential perl
set -euo pipefail

BUSYBOX_VERSION="${BUSYBOX_VERSION:-1.36.1}"
WORK="${WORK:-/tmp/kk-busybox-build}"
ARTIFACT_DIR="${ARTIFACT_DIR:-$(cd "$(dirname "$0")/.." && pwd)/artifacts}"

mkdir -p "$WORK" "$ARTIFACT_DIR"

if [ ! -d "$WORK/busybox-${BUSYBOX_VERSION}" ]; then
    echo ">>> 下载 busybox ${BUSYBOX_VERSION}..."
    cd "$WORK"
    # 不加 -q, --show-progress 本身就是进度输出,两者冲突会让 wget 退码非 0
    wget --tries=3 --timeout=60 --show-progress \
        "https://busybox.net/downloads/busybox-${BUSYBOX_VERSION}.tar.bz2" \
        -O "busybox-${BUSYBOX_VERSION}.tar.bz2"
    tar -xjf "busybox-${BUSYBOX_VERSION}.tar.bz2"
fi

cd "$WORK/busybox-${BUSYBOX_VERSION}"

echo ">>> make defconfig..."
make defconfig 2>&1 | tail -3

# 启用 https 支持的 wget + 常用工具
sed -i \
    -e 's/^# CONFIG_STATIC is not set/CONFIG_STATIC=y/' \
    -e 's/^# CONFIG_FEATURE_WGET_HTTPS is not set/CONFIG_FEATURE_WGET_HTTPS=y/' \
    -e 's/^# CONFIG_FEATURE_WGET_OPENSSL is not set/CONFIG_FEATURE_WGET_OPENSSL=y/' \
    -e 's/^# CONFIG_FEATURE_WGET_LONG_OPTIONS is not set/CONFIG_FEATURE_WGET_LONG_OPTIONS=y/' \
    -e 's/^# CONFIG_FEATURE_WGET_STATUSBAR is not set/CONFIG_FEATURE_WGET_STATUSBAR=y/' \
    -e 's/^# CONFIG_FEATURE_WGET_AUTHENTICATION is not set/CONFIG_FEATURE_WGET_AUTHENTICATION=y/' \
    -e 's/^# CONFIG_FEATURE_WGET_FTP is not set/CONFIG_FEATURE_WGET_FTP=y/' \
    .config

# 强制开静态
sed -i 's/^CONFIG_STATIC=.*/CONFIG_STATIC=y/' .config

echo ">>> make -j$(nproc)..."
make -j"$(nproc)" 2>&1 | tail -5

cp busybox "$ARTIFACT_DIR/busybox"
# v0.1.2: 确认复制成功
if [ ! -x "$ARTIFACT_DIR/busybox" ]; then
    echo "!!! 严重错误: busybox 复制后不可执行" >&2
    exit 1
fi
# --help -h 在某些 busybox 静态编译版本下会找不到 help applet，
# 单独跑 --help 通常没事；用 || true 兜底防止 set -e 把脚本弄挂
"$ARTIFACT_DIR/busybox" --help 2>&1 | head -3 || true

echo ">>> busybox 已生成: $ARTIFACT_DIR/busybox ($(du -h "$ARTIFACT_DIR/busybox" | cut -f1))"
