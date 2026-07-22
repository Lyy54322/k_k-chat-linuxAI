# 更新日志 / Changelog

## [v0.1.2] - 2026-07-22

### 🐛 修复
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
