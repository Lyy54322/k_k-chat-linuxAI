#!/usr/bin/env bash
# scripts/build_initramfs.sh
# 把 busybox + k_K 二进制 + init 脚本打成 initramfs.cpio.xz
#
# 输出: artifacts/initramfs.cpio.xz  (~6-8 MB)
#
# 依赖: 在此之前必须先运行:
#   scripts/build_busybox.sh  -> artifacts/busybox
#   cargo build --release     -> artifacts/k_K (从 k_K/ 目录跑)
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
WORK="${WORK:-/tmp/kk-initramfs}"
ARTIFACT_DIR="${ARTIFACT_DIR:-$ROOT/artifacts}"
BUSYBOX_BIN="$ARTIFACT_DIR/busybox"
K_K_BIN="${K_K_BIN:-$ARTIFACT_DIR/k_K}"

if [ ! -x "$BUSYBOX_BIN" ]; then
    echo "!!! 错误: 找不到 $BUSYBOX_BIN,先跑 scripts/build_busybox.sh" >&2
    exit 1
fi
if [ ! -x "$K_K_BIN" ]; then
    echo "!!! 错误: 找不到 $K_K_BIN,先跑 cargo build --release" >&2
    exit 1
fi

rm -rf "$WORK"
mkdir -p "$WORK"/{bin,sbin,usr/bin,usr/sbin,etc/k_K,proc,sys,dev,run,tmp,root}

# 1. 装 busybox + 必要软链接
cp "$BUSYBOX_BIN" "$WORK/bin/busybox"
chmod +x "$WORK/bin/busybox"
for tool in sh ash mount umount switch_root reboot poweroff halt init \
            ls cat cp mv rm mkdir rmdir ln echo grep sed awk \
            chmod chown chroot env export stty clear reset wc head tail \
            dd df du ps kill sleep sync test true false \
            hostname dmesg mountpoint \
            wget; do
    ln -sf /bin/busybox "$WORK/bin/$tool"
done
ln -sf /bin/busybox "$WORK/sbin/init"

# 2. 装 k_K 主程序
install -m 0755 "$K_K_BIN" "$WORK/usr/bin/k_K"

# 3. 装 init 脚本
install -m 0755 "$ROOT/rootfs/init" "$WORK/init"

# 4. 装默认配置 (允许空字符串,因为用户首启需要填写 API key)
if [ -f "$ROOT/k_K/config.txt" ]; then
    install -m 0644 "$ROOT/k_K/config.txt" "$WORK/etc/k_K/config.txt"
else
    cat > "$WORK/etc/k_K/config.txt" <<'EOF'
# k_K chat linuxAI 配置文件
# 首次启动后,程序主菜单可输入 s 编辑此文件
api_base = https://api.openai.com/v1
api_key = sk-请在这里填入你的API密钥
model_id = gpt-3.5-turbo
system_prompt = 你是一个简洁的中文助手,回答控制在100字以内。
ai_name = AI助手
EOF
    chmod 0644 "$WORK/etc/k_K/config.txt"
fi

# 5. 装 unifont 字体 (如果存在)
if [ -f "$ARTIFACT_DIR/unifont.pf2" ]; then
    mkdir -p "$WORK/usr/share/fonts"
    cp "$ARTIFACT_DIR/unifont.pf2" "$WORK/usr/share/fonts/unifont.pf2"
fi

# 6. /etc/passwd / /etc/group (busybox 需要)
cat > "$WORK/etc/passwd" <<'EOF'
root:x:0:0:root:/root:/bin/sh
EOF
cat > "$WORK/etc/group" <<'EOF'
root:x:0:
EOF

# 7. /etc/fstab (留空, init 自己 mount)
: > "$WORK/etc/fstab"

# 8. cpio 打包 + xz 压缩
cd "$WORK"
echo ">>> 生成 initramfs.cpio.xz..."
find . -print0 | cpio --null -ov --format=newc 2>/dev/null \
    | xz -9 --check=crc32 > "$ARTIFACT_DIR/initramfs.cpio.xz"

echo ">>> initramfs 已生成: $ARTIFACT_DIR/initramfs.cpio.xz ($(du -h "$ARTIFACT_DIR/initramfs.cpio.xz" | cut -f1))"
ls -la "$ARTIFACT_DIR"
