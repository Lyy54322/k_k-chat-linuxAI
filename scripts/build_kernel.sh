#!/usr/bin/env bash
# scripts/build_kernel.sh
# 编译一个最小化 Linux 内核,作为 EFI stub 启动 (bzImage.efi)
#
# 目标: 支持 framebuffer、evdev、UTF-8 console、busybox init
# 输出: artifacts/vmlinuz  (~4-6 MB)
#
# 需要 host 安装: gcc, make, bc, bison, flex, libelf-dev, libssl-dev
# Ubuntu/Debian: sudo apt install build-essential bc bison flex libelf-dev libssl-dev wget xz-utils cpio
set -euo pipefail

KERNEL_VERSION="${KERNEL_VERSION:-6.6.10}"
WORK="${WORK:-/tmp/kk-kernel-build}"
ARTIFACT_DIR="${ARTIFACT_DIR:-$(cd "$(dirname "$0")/.." && pwd)/artifacts}"

mkdir -p "$WORK" "$ARTIFACT_DIR"

if [ ! -d "$WORK/linux-${KERNEL_VERSION}" ]; then
    echo ">>> 下载 Linux ${KERNEL_VERSION} 内核源..."
    cd "$WORK"
    wget -q --show-progress \
        "https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-${KERNEL_VERSION}.tar.xz" \
        -O "linux-${KERNEL_VERSION}.tar.xz"
    tar -xJf "linux-${KERNEL_VERSION}.tar.xz"
fi

cd "$WORK/linux-${KERNEL_VERSION}"

echo ">>> 生成最小化 .config (framebuffer + evdev + EFI stub)..."
cat > .config <<'KCONFIG'
# 最小化配置: framebuffer、evdev、UTF-8、EFI stub
CONFIG_64BIT=y
CONFIG_X86_64=y
CONFIG_EFI=y
CONFIG_EFI_STUB=y
CONFIG_EFI_MIXED=n
CONFIG_EFIVARS=n
CONFIG_SYSVIPC=n
CONFIG_POSIX_MQUEUE=n
CONFIG_CGROUPS=n
CONFIG_NAMESPACES=n
CONFIG_NET=n
CONFIG_PACKET=n
CONFIG_UNIX=n
CONFIG_INET=n
CONFIG_SCSI=n
CONFIG_SATA=n
CONFIG_ATA=n
CONFIG_USB=n
CONFIG_INPUT=y
CONFIG_INPUT_EVDEV=y
CONFIG_INPUT_MISC=y
CONFIG_VT=y
CONFIG_VT_CONSOLE=y
CONFIG_HW_CONSOLE=y
CONFIG_VGA_CONSOLE=y
CONFIG_DRM=y
CONFIG_DRM_FBDEV_EMULATION=y
CONFIG_DRM_SIMPLEDRM=y
CONFIG_FB=y
CONFIG_FB_SIMPLE=y
CONFIG_FB_VESA=y
CONFIG_FB_EFI=y
CONFIG_FRAMEBUFFER_CONSOLE=y
CONFIG_FONT=y
CONFIG_FONT_8x8=y
CONFIG_FONT_8x16=y
CONFIG_FONT_SUN12x22=y
CONFIG_FONT_SUN8x16=y
CONFIG_NLS=y
CONFIG_NLS_UTF8=y
CONFIG_NLS_CODEPAGE_437=y
CONFIG_NLS_CODEPAGE_936=y
CONFIG_TMPFS=y
CONFIG_TMPFS_POSIX_ACL=y
CONFIG_CGROUPS=n
CONFIG_BLK_DEV_INITRD=y
CONFIG_RD_GZIP=y
CONFIG_RD_XZ=y
CONFIG_BINFMT_SCRIPT=y
CONFIG_BINFMT_ELF=y
CONFIG_ELF_CORE=n
CONFIG_PROC_FS=y
CONFIG_PROC_SYSCTL=y
CONFIG_PROC_PAGE_MONITOR=n
CONFIG_SYSFS=n
CONFIG_DEBUG_KERNEL=n
CONFIG_DEBUG_INFO=n
CONFIG_MAGIC_SYSRQ=n
CONFIG_DETECT_HUNG_TASK=n
CONFIG_BOOTPARAM_HUNG_TASK_PANIC=n
CONFIG_RCU_TRACE=n
# 关闭一切省电/调度优化,镜像更小启动更快
CONFIG_SMP=n
CONFIG_SCHED_SMT=n
CONFIG_NUMA=n
CONFIG_HZ=100
KCONFIG

echo ">>> make olddefconfig (让 Kconfig 补全缺失选项)..."
make olddefconfig 2>&1 | tail -3

echo ">>> make -j$(nproc) bzImage (编译内核)..."
make -j"$(nproc)" bzImage 2>&1 | tail -10

cp arch/x86/boot/bzImage "$ARTIFACT_DIR/vmlinuz"
echo ">>> 内核已生成: $ARTIFACT_DIR/vmlinuz ($(du -h "$ARTIFACT_DIR/vmlinuz" | cut -f1))"
