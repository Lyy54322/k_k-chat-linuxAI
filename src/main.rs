mod config;
mod evdev_input;
mod fbdev;
mod handwriting;
mod hwr_engine;
mod network;
mod ui;

use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use config::AppConfig;
use fbdev::Framebuffer;
use handwriting::HandwritingState;
use hwr_engine::HwrEngine;
use network::ApiClient;
use ui::TerminalUI;

// ─── 全局统一配色 (ANSI) ──────────────────────────────────────────
mod colors {
    pub const CYAN: &str = "\x1b[96m";
    pub const WHITE: &str = "\x1b[37m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const RED: &str = "\x1b[31m";
    pub const RESET: &str = "\x1b[0m";
}

// ─── 指令处理结果 ─────────────────────────────────────────────────
enum CommandResult {
    Continue,
    Exit,
}

// ═══════════════════════════════════════════════════════════════════
//  主函数
// ═══════════════════════════════════════════════════════════════════
fn main() {
    // ── 检测硬件 ──────────────────────────────────────────────────
    let fb_available = std::path::Path::new("/dev/fb0").exists();
    let mouse_available = evdev_input::find_mouse_device().is_some();
    let use_hw = fb_available && mouse_available;

    let mut config = AppConfig::load();
    let ui = TerminalUI::new();
    let mut conversations: Vec<Vec<(String, String)>> = vec![Vec::new()];
    let mut active_conv: usize = 0;

    // ── 首次启动检查：配置不完整则引导 ──────────────────────────
    if config.api_base.is_empty() || config.api_key.is_empty() || config.model_id.is_empty() {
        ui.show_startup(&config.model_id, active_conv);
        println!(
            "{}[提示] 首次使用，请配置你的 AI 提供商信息。{}",
            colors::YELLOW, colors::RESET
        );
        println!(
            "{}支持任意 OpenAI 兼容 API（NVIDIA / OpenAI / 阿里云 / 智谱等）{}",
            colors::CYAN, colors::RESET
        );
        println!(
            "{}示例地址: https://api.openai.com/v1/chat/completions{}",
            colors::CYAN, colors::RESET
        );
        println!(
            "{}示例模型: gpt-4o / gpt-3.5-turbo / llama-3.1-405b-instruct{}",
            colors::CYAN, colors::RESET
        );
        println!(
            "{}输入 /setting 或直接按提示配置即可{}",
            colors::CYAN, colors::RESET
        );
        println!();
    } else {
        ui.show_startup(&config.model_id, active_conv);
    }

    // ── 初始化手写子系统 ──────────────────────────────────────────
    let hw_state = if use_hw {
        match HandwritingState::new() {
            Ok(mut s) => {
                let top = {
                    let fb = Framebuffer::open().ok();
                    fb.map(|f| f.canvas_top).unwrap_or(0)
                };
                s.set_canvas_top(top);
                println!("{}[信息] 手写画板已启用{}", colors::CYAN, colors::RESET);
                Some(Arc::new(Mutex::new(s)))
            }
            Err(e) => {
                eprintln!("{}[警告] 无法启用手写画板: {}，自动切换纯文本模式{}", colors::RED, e, colors::RESET);
                None
            }
        }
    } else {
        println!("{}[信息] 手写画板不可用，纯文本模式运行{}", colors::YELLOW, colors::RESET);
        None
    };

    let hwr_engine = if hw_state.is_some() {
        match HwrEngine::new() {
            Ok(eng) => Some(Arc::new(Mutex::new(eng))),
            Err(_) => None,
        }
    } else {
        None
    };

    let hw_running = Arc::new(AtomicBool::new(true));

    // ── 启动手写线程 ────────────────────────────────────────────
    let hw_state_clone = hw_state.clone();
    let hw_engine_clone = hwr_engine.clone();
    let hw_running_clone = hw_running.clone();
    if hw_state.is_some() {
        let _ = thread::spawn(move || {
            handwriting::handwriting_loop(hw_state_clone, hw_engine_clone, hw_running_clone);
        });
    }

    // ── 主交互循环 ────────────────────────────────────────────────
    let mut input_buf = String::new();
    let mut candidates: Vec<String> = Vec::new();
    let mut selected_cand: usize = 0;
    let showing_candidates = AtomicBool::new(false);

    loop {
        // 检查手写识别结果
        if let Some(ref state) = hw_state {
            let mut s = state.lock().unwrap();
            if !s.pending_candidates.is_empty() {
                candidates = std::mem::take(&mut s.pending_candidates);
                selected_cand = 0;
                showing_candidates.store(true, Ordering::SeqCst);
                drop(s);
                ui.show_candidates(&candidates, selected_cand);
                continue;
            }
        }

        ui.show_prompt_with_input(&input_buf);
        if showing_candidates.load(Ordering::SeqCst) {
            ui.show_candidates(&candidates, selected_cand);
        }

        let mut line = String::new();
        match read_line_with_timeout(&mut line, Duration::from_millis(200)) {
            Ok(true) => {
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() && !showing_candidates.load(Ordering::SeqCst) {
                    continue;
                }

                // 候选字选择
                if showing_candidates.load(Ordering::SeqCst) {
                    match trimmed.as_str() {
                        "\x1b[A" => {
                            if selected_cand > 0 {
                                selected_cand -= 1;
                                ui.show_candidates(&candidates, selected_cand);
                            }
                            continue;
                        }
                        "\x1b[B" => {
                            if selected_cand < candidates.len().saturating_sub(1) {
                                selected_cand += 1;
                                ui.show_candidates(&candidates, selected_cand);
                            }
                            continue;
                        }
                        _ => {
                            if !candidates.is_empty() && selected_cand < candidates.len() {
                                input_buf.push_str(&candidates[selected_cand]);
                            }
                            candidates.clear();
                            showing_candidates.store(false, Ordering::SeqCst);
                            ui.show_prompt_with_input(&input_buf);
                            continue;
                        }
                    }
                }

                let full_input = if input_buf.is_empty() {
                    trimmed.clone()
                } else {
                    let combined = format!("{}{}", input_buf, trimmed);
                    input_buf.clear();
                    combined
                };

                if full_input.starts_with('/') {
                    match process_command(
                        &full_input,
                        &mut config,
                        &ui,
                        &mut conversations,
                        &mut active_conv,
                        &hw_state,
                    ) {
                        CommandResult::Continue => continue,
                        CommandResult::Exit => break,
                    }
                } else {
                    send_and_display(full_input, &config, &mut conversations, active_conv, &ui);
                }
            }
            Ok(false) => continue, // timeout
            Err(_) => break,       // Ctrl+C
        }
    }

    // ── 退出清理 ────────────────────────────────────────────────
    hw_running.store(false, Ordering::SeqCst);
    println!("\n{}k_K Chat 已退出，输入 reboot 关机或继续使用 busybox shell{}", colors::CYAN, colors::RESET);
}

// ═══════════════════════════════════════════════════════════════════
//  超时行读取（让主循环可以轮询手写结果）
// ═══════════════════════════════════════════════════════════════════
fn read_line_with_timeout(buf: &mut String, timeout: Duration) -> Result<bool, io::Error> {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let fd = handle.as_raw_fd();

    let orig_flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    unsafe { libc::fcntl(fd, libc::F_SETFL, orig_flags | libc::O_NONBLOCK); }

    let start = std::time::Instant::now();
    let mut bytes = [0u8; 1];

    loop {
        match handle.read(&mut bytes) {
            Ok(1) => {
                let ch = bytes[0] as char;
                match ch {
                    '\n' | '\r' => {
                        unsafe { libc::fcntl(fd, libc::F_SETFL, orig_flags); }
                        return Ok(true);
                    }
                    '\x03' => {
                        unsafe { libc::fcntl(fd, libc::F_SETFL, orig_flags); }
                        return Err(io::Error::new(io::ErrorKind::Interrupted, "Ctrl+C"));
                    }
                    '\x1b' => {
                        // 读取转义序列
                        let mut seq = String::new();
                        seq.push(ch);
                        let deadline = std::time::Instant::now() + Duration::from_millis(50);
                        loop {
                            match handle.read(&mut bytes) {
                                Ok(1) => {
                                    seq.push(bytes[0] as char);
                                    if seq.len() >= 3 { break; }
                                }
                                Ok(_) | Err(_) => {
                                    if std::time::Instant::now() >= deadline { break; }
                                    thread::sleep(Duration::from_millis(5));
                                }
                            }
                        }
                        if seq.starts_with("\x1b[") {
                            let code = &seq[2..];
                            if code == "A" { buf.push('\x1b'); buf.push('['); buf.push('A'); }
                            else if code == "B" { buf.push('\x1b'); buf.push('['); buf.push('B'); }
                        }
                        continue;
                    }
                    '\x7f' | '\x08' => { buf.pop(); continue; }
                    _ => { buf.push(ch); continue; }
                }
            }
            Ok(0) => {
                unsafe { libc::fcntl(fd, libc::F_SETFL, orig_flags); }
                return Ok(true);
            }
            Ok(_) => continue,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                if start.elapsed() >= timeout {
                    unsafe { libc::fcntl(fd, libc::F_SETFL, orig_flags); }
                    return Ok(false);
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                unsafe { libc::fcntl(fd, libc::F_SETFL, orig_flags); }
                return Err(e);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  发送消息并显示回复
// ═══════════════════════════════════════════════════════════════════
fn send_and_display(
    msg: String,
    config: &AppConfig,
    conversations: &mut Vec<Vec<(String, String)>>,
    active_conv: usize,
    ui: &TerminalUI,
) {
    println!("{}你:{} {}", colors::CYAN, colors::RESET, msg);
    conversations[active_conv].push(("user".to_string(), msg));

    print!("{}思考中...{}", colors::YELLOW, colors::RESET);
    io::stdout().flush().ok();

    let mut client = ApiClient::new(&config.api_base, &config.api_key, &config.model_id, &config.system_prompt);
    let history: Vec<(String, String)> = conversations[active_conv].clone();

    match client.send_message(&history) {
        Ok(response) => {
            print!("\r\x1b[2K");
            println!("{}AI:{} {}", colors::WHITE, colors::RESET, response);
            conversations[active_conv].push(("assistant".to_string(), response));
        }
        Err(e) => {
            print!("\r\x1b[2K");
            eprintln!("{}{}", colors::RED, e);
        }
    }

    ui.show_status_bar(&config.model_id, active_conv);
    ui.show_prompt();
}

// ═══════════════════════════════════════════════════════════════════
//  指令分发
// ═══════════════════════════════════════════════════════════════════
fn process_command(
    cmd: &str,
    config: &mut AppConfig,
    ui: &TerminalUI,
    conversations: &mut Vec<Vec<(String, String)>>,
    active_conv: &mut usize,
    hw_state: &Option<Arc<Mutex<HandwritingState>>>,
) -> CommandResult {
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    let command = parts[0];

    match command {
        "/help" => { ui.show_help(); CommandResult::Continue }
        "/setting" => { settings_menu(config, ui); CommandResult::Continue }
        "/tabs" => { tab_menu(conversations, active_conv, ui); CommandResult::Continue }
        "/clear" => {
            if *active_conv < conversations.len() {
                conversations[*active_conv].clear();
            }
            println!("{}当前对话上下文已清空{}", colors::YELLOW, colors::RESET);
            CommandResult::Continue
        }
        "/clearboard" => {
            if let Some(state) = hw_state {
                let mut s = state.lock().unwrap();
                s.strokes.clear();
                s.needs_redraw = true;
            }
            println!("{}手写画布已清空{}", colors::YELLOW, colors::RESET);
            CommandResult::Continue
        }
        "/exit" => CommandResult::Exit,
        _ => {
            println!("{}[错误] 未知指令: {}，输入 /help 查看可用指令{}", colors::RED, command, colors::RESET);
            CommandResult::Continue
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  设置菜单
// ═══════════════════════════════════════════════════════════════════
fn settings_menu(config: &mut AppConfig, ui: &TerminalUI) {
    loop {
        ui.clear_screen();
        println!("{}===== 设置菜单 ====={}", colors::CYAN, colors::RESET);
        let base_display = if config.api_base.is_empty() {
            "(未设置)".to_string()
        } else {
            config.api_base.clone()
        };
        println!("1. 修改 API 地址 (当前: {})", base_display);
        println!(
            "2. 修改 System Prompt (当前: {})",
            if config.system_prompt.is_empty() { "(空白)" } else { &config.system_prompt }
        );
        println!("3. 更换调用模型 (当前: {})", if config.model_id.is_empty() { "(未设置)" } else { &config.model_id });
        let key_display = if config.api_key.is_empty() {
            "(未设置)".to_string()
        } else if config.api_key.len() > 12 {
            format!("{}...{}", &config.api_key[..8], &config.api_key[config.api_key.len()-4..])
        } else {
            "****".to_string()
        };
        println!("4. 更换 API 密钥 (当前: {})", key_display);
        println!(
            "5. 修改 AI 对我的专属称呼 (当前: {})",
            if config.ai_name.is_empty() { "(无)" } else { &config.ai_name }
        );
        println!("6. 保存全部配置至 config.txt（永久生效）");
        println!("0. 返回聊天主界面");
        print!("{}> {}", colors::CYAN, colors::RESET);
        io::stdout().flush().ok();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).ok();
        let choice = choice.trim();

        match choice {
            "1" => {
                println!("{}支持任意 OpenAI 兼容 API{}", colors::CYAN, colors::RESET);
                println!("{}示例: https://api.openai.com/v1/chat/completions{}", colors::CYAN, colors::RESET);
                print!("请输入新的 API 地址 (直接回车保持不变): ");
                io::stdout().flush().ok();
                let mut val = String::new();
                io::stdin().read_line(&mut val).ok();
                let val = val.trim().to_string();
                if !val.is_empty() { config.api_base = val; }
            }
            "2" => {
                print!("请输入新的 System Prompt (直接回车保持不变): ");
                io::stdout().flush().ok();
                let mut val = String::new();
                io::stdin().read_line(&mut val).ok();
                let val = val.trim().to_string();
                if !val.is_empty() { config.system_prompt = val; }
            }
            "3" => model_select_menu(config),
            "4" => {
                println!("{}提示: 请前往你的 API 提供商控制台获取密钥{}", colors::YELLOW, colors::RESET);
                print!("请输入新的 API 密钥: ");
                io::stdout().flush().ok();
                let mut val = String::new();
                io::stdin().read_line(&mut val).ok();
                let val = val.trim().to_string();
                if !val.is_empty() { config.api_key = val; }
            }
            "5" => {
                print!("请输入 AI 对你的专属称呼: ");
                io::stdout().flush().ok();
                let mut val = String::new();
                io::stdin().read_line(&mut val).ok();
                let val = val.trim().to_string();
                if !val.is_empty() { config.ai_name = val; }
            }
            "6" => match config.save() {
                Ok(_) => println!("{}配置已保存至 config.txt{}", colors::CYAN, colors::RESET),
                Err(e) => eprintln!("{}保存失败: {}{}", colors::RED, e, colors::RESET),
            },
            "0" => break,
            _ => println!("{}无效选项{}", colors::RED, colors::RESET),
        }
    }
    ui.clear_screen();
}

// ═══════════════════════════════════════════════════════════════════
//  模型选择（通用示例，涵盖多提供商）
// ═══════════════════════════════════════════════════════════════════
fn model_select_menu(config: &mut AppConfig) {
    let models = [
        ("gpt-4o",                  "OpenAI GPT-4o"),
        ("gpt-3.5-turbo",           "OpenAI GPT-3.5"),
        ("nvidia/nemotron-3-ultra", "NVIDIA Nemotron 3 Ultra"),
        ("meta/llama-3.1-405b-instruct", "Meta Llama 3.1 405B"),
        ("mistralai/mixtral-8x22b-v0.1", "Mistral Mixtral 8x22B"),
        ("deepseek-ai/deepseek-v2-chat", "DeepSeek V2"),
        ("qwen-plus",               "阿里云通义千问 Plus"),
        ("glm-4",                   "智谱 GLM-4"),
        ("gemini-1.5-pro",          "Google Gemini 1.5 Pro"),
    ];

    loop {
        println!("\n{}===== 模型选择列表（示例）====={}", colors::CYAN, colors::RESET);
        println!("{}提示: 以下仅为示例，实际可用模型取决于你的 API 提供商{}", colors::YELLOW, colors::RESET);
        for (i, (id, name)) in models.iter().enumerate() {
            let current = if *id == config.model_id { " ◄当前" } else { "" };
            println!("{}. {:25}({}){}", i + 1, name, id, current);
        }
        println!("0. 手动输入自定义模型ID");
        print!("{}> {}", colors::CYAN, colors::RESET);
        io::stdout().flush().ok();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).ok();
        let choice = choice.trim();

        if choice == "0" {
            print!("请输入模型ID: ");
            io::stdout().flush().ok();
            let mut val = String::new();
            io::stdin().read_line(&mut val).ok();
            let val = val.trim().to_string();
            if !val.is_empty() {
                config.model_id = val;
                println!("{}已切换至: {}{}", colors::CYAN, config.model_id, colors::RESET);
            }
            break;
        } else if let Ok(idx) = choice.parse::<usize>() {
            if idx >= 1 && idx <= models.len() {
                config.model_id = models[idx - 1].0.to_string();
                println!("{}已切换至: {}{}", colors::CYAN, config.model_id, colors::RESET);
                break;
            }
        }
        println!("{}无效选项{}", colors::RED, colors::RESET);
    }
}

// ═══════════════════════════════════════════════════════════════════
//  多对话标签页管理
// ═══════════════════════════════════════════════════════════════════
fn tab_menu(
    conversations: &mut Vec<Vec<(String, String)>>,
    active_conv: &mut usize,
    ui: &TerminalUI,
) {
    loop {
        println!("\n{}===== 对话标签管理 ====={}", colors::CYAN, colors::RESET);
        println!("当前激活对话：第{}号会话", *active_conv + 1);
        println!("1. 新建空白对话（全新上下文）");
        println!("2. 切换已有对话（输入编号跳转）");
        println!("3. 删除指定对话（输入编号删除）");
        println!("4. 列出全部已创建会话");
        println!("0. 返回聊天主界面");
        print!("{}> {}", colors::CYAN, colors::RESET);
        io::stdout().flush().ok();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).ok();
        let choice = choice.trim();

        match choice {
            "1" => {
                conversations.push(Vec::new());
                *active_conv = conversations.len() - 1;
                println!("{}已创建第{}号对话{}", colors::CYAN, *active_conv + 1, colors::RESET);
            }
            "2" => {
                print!("输入要切换的对话编号: ");
                io::stdout().flush().ok();
                let mut num = String::new();
                io::stdin().read_line(&mut num).ok();
                if let Ok(idx) = num.trim().parse::<usize>() {
                    if idx >= 1 && idx <= conversations.len() {
                        *active_conv = idx - 1;
                        println!("{}已切换至第{}号对话{}", colors::CYAN, idx, colors::RESET);
                    }
                }
            }
            "3" => {
                if conversations.len() <= 1 {
                    println!("{}至少保留一个对话{}", colors::RED, colors::RESET);
                    continue;
                }
                print!("输入要删除的对话编号: ");
                io::stdout().flush().ok();
                let mut num = String::new();
                io::stdin().read_line(&mut num).ok();
                if let Ok(idx) = num.trim().parse::<usize>() {
                    if idx >= 1 && idx <= conversations.len() {
                        conversations.remove(idx - 1);
                        if *active_conv >= conversations.len() {
                            *active_conv = conversations.len() - 1;
                        } else if *active_conv >= idx.saturating_sub(1) && *active_conv > 0 && idx - 1 <= *active_conv {
                            // no-op
                        }
                        println!("{}已删除第{}号对话{}", colors::CYAN, idx, colors::RESET);
                    }
                }
            }
            "4" => {
                println!("\n{}--- 已创建会话列表 ---{}", colors::CYAN, colors::RESET);
                for (i, conv) in conversations.iter().enumerate() {
                    let marker = if i == *active_conv { " ◄当前" } else { "" };
                    println!("{}. 第{}号会话 ({}条消息){}", i + 1, i + 1, conv.len(), marker);
                }
            }
            "0" => break,
            _ => println!("{}无效选项{}", colors::RED, colors::RESET),
        }
    }
}