# 更新日志 / Changelog

## [v0.1.2] - 2026-07-23 (Round 2 二次审核)

### 🐛 修复

- **方向键 50ms 卡顿** (`k_K/src/main.rs`)
  - 旧版 `read_line_with_timeout` 的转义序列循环漏判 3 字节且含 `[` 且结尾是字母的序列。
  - 实际触发场景：按 `↑` / `↓` 切换候选字时，每按一次会卡 50ms 才返回（`deadline` 兜底）。
  - 新增第 4 条 break 条件：`seq.len() >= 3 && seq.contains('[') && seq.ends_with(is_ascii_alphabetic)`。
  - 顺手把 `c == '~'` 限制从第 4 条移除，让 `\x1b[15~` 这种 5 字节 F5 序列能正确走到第 5 字节才 break。

- **手写识别无匹配时静默失败** (`k_K/src/handwriting.rs` + `k_K/src/main.rs`)
  - 旧版 `recognize` 返回空 Vec 时，handwriting 线程什么都不做，笔画留在画布上但用户无任何提示。
  - 新增 `no_match_pending: AtomicBool`：handwriting 线程识别失败时置位；主线程每轮轮询，发现置位就打印一行 `"提示: 未识别出候选字, 请在画板上继续写或重试"` 并清位。
  - 笔画不擦除，用户可在原笔画上继续写以增加特征。

- **initramfs 静默打包本地 `k_K/config.txt`** (`scripts/build_initramfs.sh`)
  - 旧版：`if [ -f "$ROOT/k_K/config.txt" ]` 存在则打包到 initramfs。
  - 风险：开发者本地的 `config.txt` 含真实 API key，**会直接进入发行的 ISO/EFI**，随镜像分发到所有下载者。
  - 修复：永远使用占位符配置；如检测到本地 config，打印警告告知不会被打包。

- **release.yml 源码 7z 与 zip 不一致** (`.github/workflows/release.yml`)
  - 旧版 source.7z 只打包 `k_K/src`、`k_K/Cargo.toml`、`scripts/`、`boot/`、`rootfs/`、`k_K/config.txt`。
  - 缺：`README.md`、`CHANGELOG.md`、`LICENSE`、`.github/`、`.gitignore`、根目录其他文件。
  - 修复：source.7z 改为从仓库根目录全量打包，排除 `artifacts/`、`k_K/target/`、`.git/`、`k_K/config.txt`，与 source.zip 完全对齐。

- **`build_all.sh` 编译失败被管道吞掉** (`scripts/build_all.sh`)
  - 旧版 `cargo build ... 2>&1 | tail -5` 走 `if` 时，pipeline 退码是 `tail` 的 0，cargo 编译失败也被当成成功。
  - 后续 `cp` 找不到产物文件时才报错，错误信息误导。
  - 修复：用 `${PIPESTATUS[0]}` 拿 cargo 真实退码；编译后 `if [ ! -x "$ARTIFACT_DIR/k_K" ]` 强校验产物存在；cargo/rustc 都缺时直接 exit 1。

- **busybox 复制后未校验** (`scripts/build_busybox.sh`)
  - 旧版 `cp busybox "$ARTIFACT_DIR/busybox"` 之后直接 `--help` 跑一遍，文件不存在会报模糊错误。
  - 修复：复制后 `[ -x ... ]` 强校验，不存在直接 exit 1。

- **kernel .config 重复 `CONFIG_CGROUPS=n`** (`scripts/build_kernel.sh`)
  - 旧版 heredoc 写了两次 `CONFIG_CGROUPS=n`（中间隔了 `CONFIG_BLK_DEV_INITRD` 等）。
  - 修复：删掉第二次出现的重复行。

- **GRUB 启动报 `locale` 目录警告** (`boot/grub/grub.cfg`)
  - 旧版有 `set locale_dir=$prefix/locale` 和 `set lang=zh_CN`，但 `grub-mkstandalone` 没打包 locale 目录，启动时打印 `file not found` 警告。
  - 修复：删掉这两行，GRUB 走默认英文菜单，中文渲染由 `unifont.pf2` 兜底。

### ✨ 行为改进

- **退出码 0 优先**：如果 `cargo` 编译失败但 `tail` 成功，旧版会进 then 分支走错误的 cp 路径；新版用 `PIPESTATUS` 正确判定失败后回退 gnu 构建。
- **API key 永不出 initramfs**：本地 `k_K/config.txt` 永远不会被 `build_initramfs.sh` 读取。

### 🔄 变更

- `k_K/src/handwriting.rs` 新增字段 `no_match_pending: AtomicBool`，公开读、私有写。
- `k_K/src/main.rs` 主循环轮询 `no_match_pending`，命中时打印一行黄字提示。

### 📦 推荐下一版号

- `v0.1.2`（patch 级别，仅 bug 修复 + 行为改进，不引入新功能）

---

## [v0.1.1] - 2026-07-22 (Round 1)

### 🐛 修复
- **手写识别模板**：`hwr_engine.rs` 的 `char_to_template` 从 `DefaultHasher::new().hash(ch)` 的伪随机噪声改造为基于 Unicode 码点 + 笔画数表的稳定指纹。
- **fbdev Drop UB**：`Drop::drop` 中 `munmap` 长度从 `line_len * height` 改为保存下来的 `smem_len`。
- **候选字数字键选择**：`main.rs` 候选字分支实现 README 承诺的「数字键 1-5 快速选择」。
- **删除对话 active_conv 调整**：原版 `idx.saturating_sub(1) > 0` 条件写错；改为按 `removed_idx` 与 `active_conv` 关系正确调整。
- **手写线程 join**：退出时 `hw_running.store(false)` 之后 join 手写线程。
- **config 解析含 `=` 的值**：`split_once('=')` 只切第一个 `=`。
- **UI 候选字行残留**：`show_candidates` 用 `\r\x1b[2K` 清行覆盖。
- **hwr_engine match arm 重复字符**：清理了 `'门'` / `'要'` / `'五'` / `'行'` 等重复。
- **find_mouse_device 冗余**：`/dev/input/` 扫描移到显式查找 event0-5 之后。

### ✨ 新增
- 完整开源 UEFI 启动链构建脚本（`scripts/`）
- `rootfs/init`、`boot/grub/grub.cfg`
- `.github/workflows/release.yml`、`.github/workflows/ci.yml`
- 完整 `.gitignore`

### 🔄 变更
- `README.md` 改写为 v0.1.1 版本

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
