#!/usr/bin/env bash
# scripts/build_images.sh
# 把 vmlinuz + initramfs + grub.cfg + unifont 打成
#   1) artifacts/k_K-linuxAI-v0.1.1.efi   (UEFI 可直接启动, ~13 MB)
#   2) artifacts/k_K-linuxAI-v0.1.1.iso   (CD/USB 可启动, ~13 MB)
#
# 依赖: grub-mkstandalone, xorriso, mtools
# Ubuntu/Debian: sudo apt install grub-efi-amd64-bin xorriso mtools
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
ARTIFACT_DIR="${ARTIFACT_DIR:-$ROOT/artifacts}"
STAGE="${STAGE:-/tmp/kk-grub-stage}"
VERSION="${VERSION:-v0.1.1}"

mkdir -p "$ARTIFACT_DIR" "$STAGE/boot/grub" "$STAGE/EFI/BOOT"

# 检查依赖
for bin in grub-mkstandalone xorriso mkfs.fat mcopy mmd; do
    if ! command -v "$bin" >/dev/null 2>&1; then
        echo "!!! 缺少依赖: $bin" >&2
        echo "    Ubuntu/Debian: sudo apt install grub-efi-amd64-bin xorriso mtools dosfstools" >&2
        exit 1
    fi
done

# 复制镜像内容到 stage
echo ">>> 准备 stage 目录..."
cp "$ARTIFACT_DIR/vmlinuz"          "$STAGE/boot/vmlinuz"
cp "$ARTIFACT_DIR/initramfs.cpio.xz" "$STAGE/boot/initramfs.cpio.xz"
cp "$ROOT/boot/grub/grub.cfg"        "$STAGE/boot/grub/grub.cfg"
if [ -f "$ARTIFACT_DIR/unifont.pf2" ]; then
    cp "$ARTIFACT_DIR/unifont.pf2"   "$STAGE/boot/grub/unifont.pf2"
fi

# 1) 出 .efi (单文件 UEFI 启动镜像)
echo ">>> grub-mkstandalone 生成 EFI 镜像..."
EFI_OUT="$ARTIFACT_DIR/k_K-linuxAI-${VERSION}.efi"
grub-mkstandalone \
    -O x86_64-efi \
    -o "$EFI_OUT" \
    --modules="all_video gfxterm font gfxmenu part_gpt part_msdos iso9660 normal search linux reboot halt" \
    --locales=zh_CN \
    --themes= \
    --fonts= \
    "$STAGE/boot/grub/grub.cfg"

# 2) 出 .iso (用 xorriso 打包 El Torito 启动)
echo ">>> xorriso 生成 ISO 镜像..."
ISO_OUT="$ARTIFACT_DIR/k_K-linuxAI-${VERSION}.iso"
# 把 grub.cfg 复制到 ISO 根目录方便 BIOS/EFI 两种模式启动
cp "$STAGE/boot/grub/grub.cfg" "$STAGE/grub.cfg"
cp -r "$STAGE/boot"/* "$STAGE/"

# 准备 EFI 引导镜像
EFI_BOOT_IMG="$STAGE/EFI/BOOT/BOOTX64.EFI"
mkdir -p "$(dirname "$EFI_BOOT_IMG")"
# 用 grub-mkrescue 的等价方法: 生成 EFI 引导文件
grub-mkstandalone \
    -O x86_64-efi \
    -o "$EFI_BOOT_IMG" \
    --modules="all_video gfxterm font normal linux reboot halt" \
    --locales=zh_CN \
    --themes= \
    --fonts= \
    /boot/grub/grub.cfg 2>/dev/null \
    || cp "$EFI_OUT" "$EFI_BOOT_IMG"

# 用 grub-mkrescue (它会处理 x86_64-efi 引导)
grub-mkrescue -o "$ISO_OUT" "$STAGE" 2>&1 | tail -5

# 输出
echo ""
echo "==========================================="
echo ">>> 镜像构建完成:"
ls -la "$ARTIFACT_DIR"/*.efi "$ARTIFACT_DIR"/*.iso 2>&1
echo "==========================================="
