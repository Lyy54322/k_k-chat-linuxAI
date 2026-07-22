# k_K Chat Terminal

<p align="center">
  <strong>纯 Rust · UEFI 直启 · 离线手写 · 裸机 AI 对话</strong>
</p>

k_K Chat 是一个运行在 Linux 帧缓冲（fbdev）上的轻量级 AI 聊天终端，使用纯 Rust 开发，支持离线中文手写识别和任意 OpenAI 兼容 API 大模型对话。整个系统打包为 UEFI 可启动镜像，插入 U 盘即可在任意支持 UEFI 的 x86_64 电脑上直接启动，无需安装操作系统。

> **AI含量100% — 用 AI 写的代码，用来跟 AI 对话。**

---

## v0.1.1 更新要点

> v0.1.0 → v0.1.1 修复了多个关键 bug，详见 [CHANGELOG.md](./CHANGELOG.md)。

**主要修复**:
- 🐛 **手写识别模板** 改造（从 hash 噪声 → Unicode 码点+笔画数表稳定指纹）
- 🐛 **fbdev munmap UB** 修复（保留 smem_len）
- 🐛 **数字键 1-5 快速选择候选字** 真正实现（README 之前承诺但代码漏了）
- 🐛 **删除对话 active_conv 调整**（之前是写错的死代码）
- 🐛 **手写线程 join**（退出时正确收尾）
- 🐛 **config 含 `=` 的值**（splitn(2, '=')）
- 🐛 **UI 候选字行残留**（改覆盖不堆叠）
- ✨ **完整开源 UEFI 启动链**（scripts/ + rootfs/ + boot/）
- ✨ **GitHub Actions 自动 release**（`.github/workflows/release.yml`）

---

## 功能特性

### AI 对话
- **任意 OpenAI 兼容 API** — 支持 NVIDIA / OpenAI / 阿里云 / 智谱 / Google 等任意提供商
- **多对话标签** — 浏览器式多标签管理，对话上下文相互隔离，支持新建 / 切换 / 删除
- **System Prompt 自定义** — 可设定 AI 角色和专属称呼
- **配置持久化** — API 地址、密钥、模型、System Prompt 保存至 `config.txt`

### 手写输入
- **手写画板** — 基于 fbdev + evdev，鼠标左键按住拖动绘制白色抗锯齿笔迹
- **离线手写识别** — 纯 Rust 引擎，内置 3000+ 常用汉字字库，基于笔画特征提取 + 余弦相似度模板匹配
- **4x4 网格密度特征** — 有效区分字形结构，提升识别准确率
- **候选字选择** — 识别后弹出 5 个候选字，方向键切换 + 回车确认，数字键快速选择
- **右键撤销** — 鼠标右键撤销上一笔画
- **抖动滤波** — 内置 JitterFilter，消除鼠标抖动噪声

### 系统架构
- **纯 Rust** — 零 Python / Java / Go / Node.js 依赖，唯一外部 crate 为 `libc = "=0.2.150"`
- **全静态编译** — `RUSTFLAGS="-C target-feature=+crt-static"`，单二进制文件，无动态链接
- **UEFI 直启** — GRUB UEFI 引导 + Linux 内核 + initramfs，约 3 秒启动到可用
- **无图形依赖** — 直接操作 `/dev/fb0` 帧缓冲显存（mmap），无需 X11 / Wayland
- **统一配色** — 全屏纯黑底 + 青色（Logo/提示符）+ 黄色（标题/状态栏）+ 白色（AI 回复）+ 红色（错误），ANSI 色值
- **无缝布局** — 上方文字聊天区与下方手写画板无分隔、无窗框、浑然一体
- **纯文本回退** — 无 fbdev 或鼠标设备时自动切换纯文本模式

---

## 屏幕布局

```
┌─────────────────────────────┐
│                             │
│    ██  ██    ████           │  ← k_K Logo（青色 ASCII 像素风）
│    k_K Chat Terminal         │  ← 标题（黄色）
│    当前模型: xxx | 对话: x号  │  ← 状态栏（黄色）
│                             │
│    你: 写一首春天的诗        │  ← 用户输入（青色）
│    AI: 春风拂柳绿丝绦，     │  ← AI 回复（白色）
│        桃花含笑映溪桥。     │
│                             │
│    > ▌                      │  ← 输入提示符（青色闪烁光标）
│                             │
│                             │  ← 无分隔线，纯黑无缝过渡
│                             │
│         ······              │
│      ···  ······  ···      │  ← 手写画板（黑底白线）
│       ················      │     左键拖动写字
│         ····  ····          │     右键撤销笔画
│                             │
│  候选字: [春] 李 季 香 泰    │  ← 候选字栏（选中=青色高亮）
│  F1设置 F2标签 F3清空       │  ← 功能键栏
│         F4帮助 F5清板        │
└─────────────────────────────┘
```

---

## 快速开始

### 前置要求

- Rust stable (>= 1.75)
- Linux x86_64 系统
- 任意 OpenAI 兼容 API 的密钥（NVIDIA / OpenAI / 阿里云 / 智谱等）

### 编译

```bash
# 克隆项目
git clone https://github.com/Lyy54322/k_k-chat-linuxAI.git
cd k_k-chat-linuxAI/k_K

# Release 编译（glibc 部分静态）
RUSTFLAGS="-C target-feature=+crt-static" cargo build --release

# 产物位于 target/release/kk_chat
```

### 首次运行

1. 将编译产物 `target/release/kk_chat` 拷贝到目标 Linux 系统
2. 确保有 `/dev/fb0` 和鼠标设备（可选，无则自动切换纯文本模式）
3. 运行 `./kk_chat`
4. 首次启动会自动提示配置 API 地址、模型、密钥
5. 配置完成后开始对话

### 构建 UEFI 启动镜像

完整开源！所有脚本在 `scripts/` 目录下：

```bash
# 一键全量构建 (k_K -> 内核 -> busybox -> unifont -> initramfs -> EFI/ISO)
sudo apt install build-essential grub-efi-amd64-bin xorriso mtools
git clone https://github.com/Lyy54322/k_k-chat-linuxAI.git
cd k_k-chat-linuxAI
bash scripts/build_all.sh
```

各步骤可单独执行：
| 脚本 | 作用 | 输出 |
|------|------|------|
| `scripts/build_kernel.sh` | 编译 Linux 6.6 内核（最小化） | `artifacts/vmlinuz` |
| `scripts/build_busybox.sh` | 编译静态 busybox | `artifacts/busybox` |
| `scripts/fetch_unifont.sh` | 下载 unifont 字体（CJK） | `artifacts/unifont.pf2` |
| `scripts/build_initramfs.sh` | 打 initramfs.cpio.xz | `artifacts/initramfs.cpio.xz` |
| `scripts/build_images.sh` | grub-mkstandalone + xorriso | `artifacts/*.efi`, `artifacts/*.iso` |
| `scripts/build_all.sh` | 上面所有 | 所有 artifacts |

产物：
| 文件 | 说明 | 大小 |
|------|------|------|
| `k_K-linuxAI-v0.1.1.efi` | UEFI 单文件启动镜像 | ~13 MB |
| `k_K-linuxAI-v0.1.1.iso` | 可引导 ISO（推荐方式） | ~13 MB |

### ISO 使用方法

1. 将 `k_K-linuxAI-v0.1.1.iso` 写入 U 盘（推荐使用 Rufus / balenaEtcher，选择 DD 模式）
2. 插入目标电脑，开机按 F12 / Del / Esc 进入 BIOS 启动菜单
3. 选择 U 盘启动（UEFI 模式）
4. 约 3 秒后自动进入 k_K Chat Terminal

> 如果目标电脑仅支持 EFI 文件启动：将 `k_K-linuxAI-v0.1.1.efi` 重命名为 `BOOTX64.EFI`，放入 FAT32 格式 U 盘的 `EFI/BOOT/` 目录。

---

## 项目结构

```
k_k-chat-linuxAI/
├── .github/workflows/        # GitHub Actions CI/CD
│   ├── ci.yml                #   PR/push 跑 cargo check
│   └── release.yml           #   tag 触发自动编译+推 release
├── boot/grub/
│   └── grub.cfg              # GRUB 引导配置（含 CJK unifont）
├── rootfs/
│   └── init                  # busybox init 脚本
├── scripts/                  # 完整开源的 UEFI 构建链
│   ├── build_all.sh          #   一键全量构建
│   ├── build_busybox.sh      #   静态 busybox
│   ├── build_images.sh       #   grub-mkstandalone + xorriso
│   ├── build_initramfs.sh    #   initramfs.cpio.xz
│   ├── build_kernel.sh       #   Linux 内核
│   └── fetch_unifont.sh      #   CJK 字体
├── k_K/                      # Rust 主程序
│   ├── Cargo.toml            #   Rust 项目配置（唯一依赖: libc）
│   └── src/
│       ├── main.rs           #   主程序入口、指令分发、对话循环
│       ├── config.rs         #   配置加载/保存
│       ├── fbdev.rs          #   帧缓冲驱动（mmap /dev/fb0）
│       ├── evdev_input.rs    #   鼠标输入（evdev 采集 + 抖动滤波）
│       ├── handwriting.rs    #   手写画板线程循环
│       ├── hwr_engine.rs     #   离线手写识别（Unicode 码点+笔画数表）
│       ├── network.rs        #   HTTPS 请求（busybox wget）
│       └── ui.rs             #   终端 UI
├── CHANGELOG.md              # 更新日志
├── LICENSE                   # MIT 开源协议
└── README.md                 # 本文件
```

---

## 技术栈

| 层面 | 技术 | 说明 |
|------|------|------|
| 语言 | 纯 Rust | 零外部语言依赖 |
| 编译 | glibc 静态链接 | `RUSTFLAGS="-C target-feature=+crt-static"` |
| 图形 | Linux fbdev | 直接 mmap `/dev/fb0` 显存，无 X11/Wayland |
| 渲染 | Bresenham + Alpha Blending | 抗锯齿白色线条 |
| 输入 | Linux evdev | 鼠标事件采集 + 抖动滤波 |
| 网络 | busybox wget | 子进程 HTTPS 请求，无 openssl/rustls |
| JSON | 自定义解析器 | 无 serde 依赖 |
| 手写识别 | 特征提取 + 余弦相似度 | 4x4 网格密度 + 方向直方图 + 笔画数 |
| 引导 | GRUB UEFI | grub-mkstandalone / xorriso |
| 根文件系统 | initramfs | busybox-static + kk_chat + init |
| 中文字库 | unifont | 终端中文显示支持 |

---

## 指令与快捷键

### 文本指令（`/` 开头）

| 指令 | 功能 |
|------|------|
| `/help` | 查看全部功能指令列表 |
| `/setting` | 打开全局设置菜单（API 地址、密钥、模型、System Prompt、称呼） |
| `/tabs` | 多对话标签页管理（新建 / 切换 / 删除 / 列出） |
| `/clear` | 清空当前对话上下文 |
| `/clearboard` | 清空手写画布所有笔迹 |
| `/exit` | 退出聊天主程序 |

### 功能键

| 快捷键 | 功能 |
|--------|------|
| F1 | 打开设置菜单 |
| F2 | 对话标签页管理 |
| F3 | 清空当前对话上下文 |
| F4 | 全局帮助文档 |
| F5 | 清空手写画布笔迹 |

### 手写操作

| 操作 | 功能 |
|------|------|
| 鼠标左键按住拖动 | 在画布区域绘制白色笔迹 |
| 鼠标右键 | 撤销上一笔画 |
| 方向键 ↑↓ | 切换候选字 |
| 数字键 1-5 | 快速选择对应候选字 |
| 回车键 | 确认选中候选字并填入输入框 |

---

## 支持的 API 提供商示例

| 提供商 | API 地址示例 | 模型示例 |
|--------|-------------|----------|
| OpenAI | `https://api.openai.com/v1/chat/completions` | `gpt-4o`, `gpt-3.5-turbo` |
| NVIDIA NVCF | `https://integrate.api.nvidia.com/v1/chat/completions` | `nvidia/nemotron-3-ultra` |
| 阿里云百炼 | `https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions` | `qwen-plus` |
| 智谱 AI | `https://open.bigmodel.cn/api/paas/v4/chat/completions` | `glm-4` |
| Google Gemini | `https://generativelanguage.googleapis.com/v1beta/models` | `gemini-1.5-pro` |
| 自定义 | 任意 OpenAI 兼容地址 | 任意模型 ID |

> 在设置菜单中手动输入 API 地址和模型 ID 即可使用任意提供商。

---

## License

[MIT](LICENSE)

---

## 深度使用 AI,拥抱未来 🐶

本项目从一行代码到 UEFI 启动镜像,**全流程 AI 辅助开发**:

- **代码** — Rust 源码、UEFI 启动镜像、initramfs 装配 → AI 起草,作者审阅
- **文档** — 本 README 及代码注释 → AI 撰写,作者修订
- **真机验证** — 手写识别、fbdev 渲染、AI 对话、UEFI 启动 → 作者完成

> "AI 含量 100% —— 用 AI 写的代码,用来跟 AI 对话。" 🐶
