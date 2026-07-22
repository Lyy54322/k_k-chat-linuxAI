# 更新日志 / Changelog

## [v0.1.2] - 2026-07-22

### 🐛 修复
- **`main.rs` Mutex poisoning 导致 panic**：
  - 旧版所有 `lock().unwrap()` 在 panic 后 mutex 进入 poisoned 状态，后续所有 lock 都会直接 panic 导致程序崩溃。
  - 修复：所有 `lock().unwrap()` 改为 `lock().unwrap_or_else(|e| e.into_inner())`，recover poisoned mutex。
- **`config.rs` API Key 明文存储无权限保护**：
  - 保存 config 后调用 `set_permissions(0o600)`，仅 owner 可读写。
- **`config.rs` system_prompt 多行处理缺陷**：
  - 保存时 `\n` 转义为 `\\n`（单行存储），加载时还原。避免多行 system_prompt 写入配置文件时损坏格式。
- **`config.rs` 路径优先级**：
  - `load()` 优先检查 `/etc/k_K/config.txt`（运行时路径），回退到 `config.txt`（相对路径）。
- **`Cargo.toml` libc 版本约束过死**：
  - 从 `"=0.2.150"` 改为 `"^0.2.150"`，允许补丁版本升级。
  - release profile 添加 `debug = 1`，保留部分调试信息便于线上排查。
- **`network.rs` API 错误解析完全坏掉**：
  - 之前 `extract_content` 查找 `"content"` 字段提取回复，但 OpenAI 错误响应是
    `{"error":{"message":"..."}}`，根本没有 `"content"`。任何 API 错误都会被报为"未知 API 错误"。
  - 新增 `extract_error`，支持三种错误格式：
    1. `{"error":"string"}` （错误是字符串）
    2. `{"error":{"message":"...","type":"..."}}` （OpenAI 标准）
    3. `{"error":{"code":"...","message":"..."}}` （含 code 字段）
  - 加上顶层 `"message"` 字段兜底。
  - 错误信息会按内容（auth / model / rate / limit）归类，用户能看到具体原因。
- **`network.rs` 缺 wget 超时**：busybox wget 默认会一直重试 20 次，API 慢就永久卡住。
  - 加 `--timeout=30 --tries=1`。
  - 区分了多种 stderr 关键字（DNS / Connection refused / timeout）给出明确错误。
- **`evdev_input.rs` 触屏不会触发 Motion 事件**：
  - 之前 EV_ABS（绝对坐标）路径只更新 `self.x/self.y` 但 `return None`，主循环永远收不到。
  - 现在 `SYN_REPORT` 事件触发 Motion（按"报点"输出），相对鼠标照旧每次 REL 发一个。
  - 内部加了 `is_absolute_device()` 判断，避免对相对设备重复发 Motion。
- **`hwr_engine.rs` 相似度权重失衡**（最严重）：
  - 原版把 8 个特征按 0.04~0.25 分配权重，stroke_count 只占 0.15。
  - 但模板的 dir_histogram / aspect_ratio / quadrant / curvature / grid_density
    都是从码位派生的"确定性噪声"，和真实笔迹特征不在同一特征空间。
  - 整个 score 函数的真实信号只占 15% 权重，识别结果实际是"按码位散列排序"。
  - v0.1.2：stroke_count 0.55 + crossings 0.20 + dir_sim 0.10 + 模板 grid_density
    做确定性 tiebreak 0.15，弃用其他噪声特征。
  - 同笔画数字符排前面，差 1 画扣一半，差 2+ 画几乎被滤掉。
  - 阈值从 0.12 提到 0.30 配合新 score 范围。
- **`hwr_engine.rs` 字库 1255 个重复字符**：`COMMON_CHARS` 原本 4343 字符里有 1255 个
  重复（"品"出现 14 次，"便" 13 次，"权" 12 次...）。HashMap 插入时已经去重，
  但字符串和编译产物白白大了 ~30%。本版保序去重到 3088 字符。
- **`main.rs` 方向键 50ms 延迟**：
  - 之前读转义序列要走满 50ms 超时才能识别 3 字节的方向键（`\x1b[A`）。
  - 现在 `seq.len() == 3 && starts_with("\x1b[") && ends_with(方向字母)` 立即 break。
- **`release.yml` 缺 `VERSION` 环境变量**（v0.1.2 必修）：
  - 之前 `scripts/build_images.sh` 默认 `VERSION=v0.1.1`，release.yml 没传 VERSION。
  - tag v0.1.2 触发的 workflow 会生成 `k_K-linuxAI-v0.1.1.efi` 但上传步骤找
    `k_K-linuxAI-v0.1.2.*`，结果 release 资产全部找不到。
  - 本版在 "Run full build pipeline" 步加上 `VERSION: ${{ github.ref_name }}`。
  - `scripts/build_all.sh` 同步从环境变量读 VERSION，向后兼容默认值 v0.1.1。
- **手写识别无匹配时静默失败** (`k_K/src/handwriting.rs` + `k_K/src/main.rs`)：
  - 旧版 `recognize` 返回空 Vec 时，handwriting 线程什么都不做，笔画留在画布上但用户无任何提示。
  - 新增 `no_match_pending: AtomicBool`：handwriting 线程识别失败时置位；主线程每轮轮询，发现置位就打印提示并清位。
- **`initramfs` 静默打包本地 `k_K/config.txt`** (`scripts/build_initramfs.sh`)：
  - 旧版存在则打包到 initramfs，开发者本地的 config.txt 含真实 API key 会进入发行镜像。
  - 修复：永远使用占位符配置；如检测到本地 config，打印警告。
- **`release.yml` 源码 7z 与 zip 不一致** (`.github/workflows/release.yml`)：
  - 旧版 source.7z 只打包部分目录，缺 `README.md`、`CHANGELOG.md` 等。
  - 修复：source.7z 改为从仓库根目录全量打包，排除 `artifacts/`、`k_K/target/`、`.git/`、`k_K/config.txt`。
- **`build_all.sh` 编译失败被管道吞掉** (`scripts/build_all.sh`)：
  - 旧版 `cargo build ... 2>&1 | tail -5` 走 `if` 时，pipeline 退码是 `tail` 的 0，cargo 编译失败也被当成成功。
  - 修复：用 `${PIPESTATUS[0]}` 拿 cargo 真实退码；编译后强校验产物存在。
- **busybox sed 兼容性问题** (`scripts/build_busybox.sh`)：
  - 引入 `config_set()` 辅助函数（grep→sed/append），兼容非 GNU sed。
  - 编译前验证 `CONFIG_STATIC`、`CONFIG_FEATURE_WGET_HTTPS`、`CONFIG_FEATURE_WGET_OPENSSL` 已启用。
- **busybox 复制后未校验** (`scripts/build_busybox.sh`)：
  - 修复：复制后 `[ -x ... ]` 强校验，不存在直接 exit 1。
- **kernel .config 重复 `CONFIG_CGROUPS=n`** (`scripts/build_kernel.sh`)：
  - heredoc 写了两次 `CONFIG_CGROUPS=n`，修复删掉第二次出现的重复行。
- **GRUB 启动报 `locale` 目录警告** (`boot/grub/grub.cfg`)：
  - 旧版有 `set locale_dir=$prefix/locale` 和 `set lang=zh_CN`，但 grub-mkstandalone 没打包 locale 目录。
  - 修复：删掉这两行，GRUB 走默认英文菜单，中文渲染由 unifont.pf2 兜底。

### ✨ 行为改进

- **退出码 0 优先**：新版用 `PIPESTATUS` 正确判定失败后回退 gnu 构建。
- **API key 永不出 initramfs**：本地 `k_K/config.txt` 永远不会被 `build_initramfs.sh` 读取。

- **`init` 进程退出后系统挂死** (`rootfs/init`)：
  - 改为 `( while true; do k_K; sleep 3; done ) &` 后台循环，崩溃自动重启。
  - 注释掉 `loadkeys`（BusyBox 未提供此命令）。
- **`ui.rs` 双重清屏闪烁** (`k_K/src/ui.rs`)：
  - `show_logo` 不再清屏（调用方已清屏），消除启动时闪屏。
  - 候选字格式去掉末尾多余逗号；新增 `clear_candidates` 方法。
- **`network.rs` 临时文件竞态** (`k_K/src/network.rs`)：
  - 临时文件改用 PID 后缀 `/tmp/kk_chat_post_{PID}.json`，避免并发冲突。
- **`build_kernel.sh` 缺失关键内核配置** (`scripts/build_kernel.sh`)：
  - 添加 `CONFIG_SYSFS=y`、`CONFIG_DEVTMPFS=y`、`CONFIG_DEVTMPFS_MOUNT=y`、
    `CONFIG_TTY=y`、`CONFIG_INPUT_KEYBOARD=y`、`CONFIG_INPUT_MOUSE=y`、`CONFIG_PRINTK=y`。
  - 删除重复的 `CONFIG_CGROUPS=n`。
- **`main.rs` 方向键延迟 + Escape 取消候选**：
  - `ReadOutcome` 新增 `ArrowUp`/`ArrowDown`/`Escape` 变体。
  - 方向键在转义序列循环中立即识别返回（不再走 50ms 超时）。
  - Escape 键取消候选字选择，回到输入状态。
- **`release.yml` 构建流程问题**：
  - `apt-get install` 添加 `perl`（build_images.sh 需要）。
  - 传递 `VERSION` 和 `SKIP_K_K_BUILD` 环境变量。
  - EFI 烧录说明改为 FAT32 分区挂载 + cp（旧版 dd 方式不正确）。
  - 7z 源码包从仓库根目录全量打包，与 source.zip 对齐。

### 🔄 变更

- `k_K/src/handwriting.rs` 新增字段 `no_match_pending: AtomicBool`，公开读、私有写。
- `k_K/src/main.rs` 主循环轮询 `no_match_pending`，命中时打印黄色提示。
- `k_K/Cargo.toml` libc 依赖从 `"=0.2.150"` 改为 `"^0.2.150"`，release profile 加 `debug = 1`。

### 🔍 二次审核发现但未修复（标记供后续版本）

- **手写识别本质上仍是启发式模板匹配**，不是机器学习模型，准确率受限。
  - 建议 v0.2.x 接入真实样本训练（哪怕是小型 CNN），或者至少用基于
    Unicode 笔顺数据的近似模板（如 Unihan 数据库的 kTotalStrokes 字段）。
- **`init` 未加载 CJK console 字体**：内核 console 字体不含 CJK 字形，
  AI 返回的中文在 /dev/tty1 上会显示成方框。修这个需要 busybox 启用
  `setfont` 并把 unifont 转成 PSF 格式，工作量超出本轮范围。
- **`config.rs` `ai_name` 字段未注入到 system prompt**：用户配置的"AI 称呼"
  只保存在配置文件里，没有真正传给模型。需要在 `send_message` 里
  把 `ai_name` 拼到 system_prompt 末尾（或作为额外 system 消息）。
- **GCC/musl 编译差异只验证了 cargo check**，没跑过 release build 的端到端测试。
  编译期差异 (IoctlReq 类型) 已在 v0.1.1 修，但运行时差异（动态链接行为）未测。
- **Release 资产**未做完整性校验（无 SHA256SUMS），用户下载后无法验证完整性。
- **`release.yml` 的 source.zip/7z 打包逻辑** 包含 `k_K/config.txt` 但仓库没有这个文件
  （`if [ -f ... ] else ...` 走 else 分支），实际未影响但代码可读性差。

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
