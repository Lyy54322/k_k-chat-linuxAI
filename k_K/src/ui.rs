//! 终端 UI 渲染
//!
//! 启动界面（k_K Logo + 标题 + 状态栏）、
//! ANSI 配色、候选字栏、帮助文档。

use std::io::{self, Write};

pub struct TerminalUI;

// ── 颜色常量 ─────────────────────────────────────────────────────
mod c {
    pub const CYAN:   &str = "\x1b[96m";
    pub const WHITE:  &str = "\x1b[37m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const RED:    &str = "\x1b[31m";
    pub const RESET:  &str = "\x1b[0m";
}

impl TerminalUI {
    pub fn new() -> Self { TerminalUI }

    pub fn clear_screen(&self) {
        print!("\x1b[2J\x1b[H");
        io::stdout().flush().ok();
    }

    /// 8 行像素艺术 k_K Logo + 标题
    pub fn show_logo(&self) {
        let logo = [
            "    ██  ██        ████        ",
            "    ██  ██       ██████       ",
            "    ██  ██      ██    ██      ",
            "    ██████      ██    ██      ",
            "    ██  ██      ██    ██      ",
            "    ██  ██       ██████       ",
            "    ██  ██        ████        ",
            "    ─────────────────────      ",
        ];
        println!();
        for line in &logo {
            println!("{}{}{}", c::CYAN, line, c::RESET);
        }
    }

    pub fn show_startup(&self, model_id: &str, active_conv: usize) {
        self.clear_screen();
        self.show_logo();
        println!();
        println!("{}k_K Chat Terminal{}", c::CYAN, c::RESET);
        println!("{}─────────────────────────────────{}", c::YELLOW, c::RESET);
        self.show_status_bar(model_id, active_conv);
        println!();
        println!();
    }

    pub fn show_status_bar(&self, model_id: &str, active_conv: usize) {
        println!(
            "{}当前模型: {} | 对话编号: 第{}号 | 输入 /help 查看全部指令{}",
            c::YELLOW, model_id, active_conv + 1, c::RESET
        );
    }

    pub fn show_prompt(&self) {
        print!("{}> {}", c::CYAN, c::RESET);
        io::stdout().flush().ok();
    }

    pub fn show_prompt_with_input(&self, input: &str) {
        print!("\r\x1b[2K{}> {}{}{}", c::CYAN, c::WHITE, input, c::RESET);
        io::stdout().flush().ok();
    }

    pub fn show_candidates(&self, candidates: &[String], selected: usize) {
        print!("\r\x1b[2K{}候选字: {}{}", c::CYAN, c::RESET, c::WHITE);
        for (i, c) in candidates.iter().enumerate() {
            if i > 0 { print!(" "); }
            if i == selected {
                print!("{}[{}{}", c::CYAN, c, c::RESET);
            } else {
                print!("{}{}{}", c::YELLOW, c, c::RESET);
            }
            print!("]");
        }
        print!(" {}↑↓选择 数字键1-5 回车确认 Esc取消{}", c::CYAN, c::RESET);
        io::stdout().flush().ok();
    }

    pub fn clear_candidates(&self) {
        print!("\r\x1b[2K");
        io::stdout().flush().ok();
    }

    pub fn show_help(&self) {
        println!("\n{}===== 帮助文档 ====={}", c::CYAN, c::RESET);
        println!("{}文本指令（输入 / 开头）：{}", c::YELLOW, c::RESET);
        println!("  /help          查看全部功能指令列表");
        println!("  /setting       打开全局设置菜单 (F1)");
        println!("  /tabs          多对话标签页管理 (F2)");
        println!("  /clear         清空当前对话上下文 (F3)");
        println!("  /clearboard    清空手写画布所有笔迹 (F5)");
        println!("  /exit          退出聊天主程序");
        println!();
        println!("{}快捷键：{}", c::YELLOW, c::RESET);
        println!("  F1  打开设置菜单");
        println!("  F2  对话标签页管理");
        println!("  F3  清空当前对话上下文");
        println!("  F4  全局帮助文档");
        println!("  F5  清空手写画布笔迹");
        println!();
        println!("{}手写操作：{}", c::YELLOW, c::RESET);
        println!("  鼠标左键按住拖动    绘制白色笔迹（仅下半画布）");
        println!("  鼠标右键            撤销上一笔画");
        println!("  ↑↓方向键            切换候选字");
        println!("  回车键              确认选中候选字");
        println!();
        println!("{}对话操作：{}", c::YELLOW, c::RESET);
        println!("  直接输入文字         发送消息给AI模型");
        println!("  Ctrl+C              中断当前AI请求");
        println!();
    }
}