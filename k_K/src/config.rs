//! 配置管理 —— 密钥与参数的加载/持久化
//!
//! 支持任意 OpenAI 兼容 API 提供商，无硬编码密钥与地址。
//! 持久化写入同目录 config.txt，对话上下文仅存内存。

use std::fs;

pub struct AppConfig {
    pub api_base: String,
    pub api_key: String,
    pub model_id: String,
    pub system_prompt: String,
    pub ai_name: String,
    config_path: String,
}

impl AppConfig {
    pub fn load() -> Self {
        let config_path = "config.txt".to_string();
        let mut config = AppConfig {
            api_base: String::new(),          // 如 https://api.openai.com/v1/chat/completions
            api_key: String::new(),           // ← 无硬编码密钥，用户自行配置
            model_id: String::new(),          // 用户自行配置
            system_prompt: String::new(),
            ai_name: String::new(),
            config_path,
        };

        if let Ok(content) = fs::read_to_string(&config.config_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                // split_once('=') 在第一个 = 处切成 (key, value)，
                // 后面所有字符（包括 =）都会落到 value 里，不会丢
                if let Some((key, value)) = line.split_once('=') {
                    match key.trim() {
                        "api_base"      => config.api_base = value.trim().to_string(),
                        "api_key"       => config.api_key = value.trim().to_string(),
                        "model_id"      => config.model_id = value.trim().to_string(),
                        "system_prompt" => config.system_prompt = value.trim().to_string(),
                        "ai_name"       => config.ai_name = value.trim().to_string(),
                        _ => {}
                    }
                }
            }
        }

        config
    }

    pub fn save(&self) -> Result<(), String> {
        let mut content = String::new();
        content.push_str("# k_K Chat 配置文件\n");
        content.push_str(&format!("api_base={}\n", self.api_base));
        content.push_str(&format!("api_key={}\n", self.api_key));
        content.push_str(&format!("model_id={}\n", self.model_id));
        content.push_str(&format!("system_prompt={}\n", self.system_prompt));
        content.push_str(&format!("ai_name={}\n", self.ai_name));

        fs::write(&self.config_path, content)
            .map_err(|e| format!("写入配置文件失败: {}", e))
    }
}