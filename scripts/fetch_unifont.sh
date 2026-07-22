#!/usr/bin/env bash
# scripts/fetch_unifont.sh
# 下载 unifont (UTF-8 全字符字体),用于 GRUB/console 渲染中文
#
# 输出: artifacts/unifont.bdf + artifacts/unifont.pf2
#
# 备注: GRUB 自带字体比较丑,unifont 是 CJK 渲染最干净的免费字体
set -euo pipefail

ARTIFACT_DIR="${ARTIFACT_DIR:-$(cd "$(dirname "$0")/.." && pwd)/artifacts}"
mkdir -p "$ARTIFACT_DIR"

# unifont 官方 BDF 版本，grub-mkfont 可直接转换 BDF/PCF/TTF/OTF，不能直接吃 GNU .hex
UNIFONT_VERSION="16.0.02"
URL="https://ftp.gnu.org/gnu/unifont/unifont-${UNIFONT_VERSION}/unifont-${UNIFONT_VERSION}.bdf.gz"

if [ ! -f "$ARTIFACT_DIR/unifont.bdf" ]; then
    echo ">>> 下载 unifont ${UNIFONT_VERSION} (~16 MB)..."
    # 不加 -q, --show-progress 本身就是进度输出,两者冲突会让 wget 退码非 0
    wget --tries=3 --timeout=60 --show-progress "$URL" -O "$ARTIFACT_DIR/unifont.bdf.gz"
    gunzip -f "$ARTIFACT_DIR/unifont.bdf.gz"
fi

# 转换为 GRUB 支持的 pcf 或 pf2 格式
if command -v grub-mkfont >/dev/null 2>&1; then
    if [ ! -f "$ARTIFACT_DIR/unifont.pf2" ]; then
        echo ">>> 转换 unifont.bdf -> unifont.pf2 (GRUB 格式)..."
        grub-mkfont -s 16 -o "$ARTIFACT_DIR/unifont.pf2" "$ARTIFACT_DIR/unifont.bdf"
    fi
fi

echo ">>> unifont 已就绪:"
ls -la "$ARTIFACT_DIR"/unifont.* 2>&1
