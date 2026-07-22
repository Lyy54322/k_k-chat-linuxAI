#!/usr/bin/env bash
# scripts/fetch_unifont.sh
# 下载 unifont (UTF-8 全字符字体),用于 console 渲染中文
#
# 输出: artifacts/unifont.hex  (GRUB 用的 .hex 格式)
#
# 备注: GRUB 自带字体比较丑,unifont 是 CJK 渲染最干净的免费字体
set -euo pipefail

ARTIFACT_DIR="${ARTIFACT_DIR:-$(cd "$(dirname "$0")/.." && pwd)/artifacts}"
mkdir -p "$ARTIFACT_DIR"

# unifont 官方 hex 版本
UNIFONT_VERSION="16.0.02"
# 16.0.02 目录下 hex 文件叫 unifont_all-16.0.02.hex.gz (带 _all)
URL="https://ftp.gnu.org/gnu/unifont/unifont-${UNIFONT_VERSION}/unifont_all-${UNIFONT_VERSION}.hex.gz"

if [ ! -f "$ARTIFACT_DIR/unifont.hex" ]; then
    echo ">>> 下载 unifont ${UNIFONT_VERSION} (~16 MB)..."
    # 不加 -q, --show-progress 本身就是进度输出,两者冲突会让 wget 退码非 0
    wget --tries=3 --timeout=60 --show-progress "$URL" -O "$ARTIFACT_DIR/unifont.hex.gz"
    gunzip -f "$ARTIFACT_DIR/unifont.hex.gz"
fi

# 转换为 GRUB 支持的 pcf 或 pf2 格式
if command -v grub-mkfont >/dev/null 2>&1; then
    if [ ! -f "$ARTIFACT_DIR/unifont.pf2" ]; then
        echo ">>> 转换 unifont.hex -> unifont.pf2 (GRUB 格式)..."
        grub-mkfont -s 16 -o "$ARTIFACT_DIR/unifont.pf2" "$ARTIFACT_DIR/unifont.hex"
    fi
fi

echo ">>> unifont 已就绪:"
ls -la "$ARTIFACT_DIR"/unifont.* 2>&1
