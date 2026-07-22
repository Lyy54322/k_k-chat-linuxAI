# 更新日志 / Changelog

## [v0.1.1] - 2026-07-22

### 🐛 修复
- **手写识别模板**：`hwr_engine.rs` 的 `char_to_template` 从 `DefaultHasher::new().hash(ch)` 的伪随机噪声改造为基于 Unicode 码点 + 笔画数表的稳定指纹。同一字符现在每次生成的模板完全相同，不同字符之间有真正区分度。识别不再随机。
- **fbdev Drop UB**：`Drop::drop` 中 `munmap` 长度从 `line_len * height` 改为保存下来的 `smem_len`，避免 line padding 导致的未定义行为。`blend_pixel` 同步修 24bpp 通道（原本只在 32bpp 工作）。
- **候选字数字键选择**：`main.rs` 候选字分支实现 README 承诺的「数字键 1-5 快速选择」，原版只识别方向键+回车，输数字会走默认分支把当前 selected 字符填入。
- **删除对话 active_conv 调整**：原版 `idx.saturating_sub(1) > 0` 条件写错且分支是 `no-op` 注释死代码；改为按 `removed_idx < *active_conv` / `removed_idx == *active_conv` / `removed_idx > *active_conv` 三种情况正确调整。
- **手写线程 join**：退出时 `hw_running.store(false)` 之后 join 手写线程，避免程序退出时手写还在跑。
- **config 解析含 `=` 的值**：`split_once('=')` 改为 `splitn(2, '=')`，保留后面所有字符，兼容 `api_key=ABC=DEF` 这类配置。
- **UI 候选字行残留**：`show_candidates` 用 `\r\x1b[2K` 清行覆盖，不再用 `\n` 追加导致屏幕堆满。
- **hwr_engine match arm 重复字符**：清理了 `'门'` / `'要'` / `'五'` / `'行'` 等在多个 arm 里重复出现的字符。
- **find_mouse_device 冗余**：`/dev/input/` 扫描移到显式查找 event0-5 之后，避免一开始就冗余遍历。

### ✨ 新增
- 完整开源 **UEFI 启动链构建脚本**：
  - `scripts/build_kernel.sh` — 最小化 Linux 6.6 内核
  - `scripts/build_busybox.sh` — 静态 busybox
  - `scripts/build_initramfs.sh` — initramfs.cpio.xz 打包
  - `scripts/fetch_unifont.sh` — UTF-8 CJK 字体
  - `scripts/build_images.sh` — grub-mkstandalone + xorriso 出 .efi/.iso
  - `scripts/build_all.sh` — 一键全量构建
- `rootfs/init` — busybox init 脚本（PID 1，挂载伪文件系统，启动 k_K 主程序）
- `boot/grub/grub.cfg` — GRUB 配置（含 CJK unifont）
- `.github/workflows/release.yml` — tag 触发自动编译 + 推 release
- `.github/workflows/ci.yml` — PR/push 跑 cargo check 防破坏编译
- 完整 `.gitignore`

### 🔄 变更
- `README.md` 改写为 v0.1.1 版本，列出所有修复 + 自定义构建步骤

## [v0.1.0] - 2026-07-22

### ✨ 首次发布
- 离线中文手写识别引擎（hwr_engine, 3000+ 字库）
- evdev 鼠标 / 触控板输入
- fbdev 帧缓冲画布渲染
- 终端 UI + 候选字栏
- HTTPS 接入 OpenAI 兼容 API
- 配置文件持久化

### ⚠️ v0.1.0 已知问题（已在 v0.1.1 修复）
- 手写识别模板是 hash 噪声，识别结果随机
- 启动链构建脚本未开源
