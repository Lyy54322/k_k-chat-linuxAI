//! HTTPS 网络请求模块
//!
//! 通过 busybox 内置 wget 发起 HTTPS 请求到任意 OpenAI 兼容 API。
//! 支持自定义 API 地址、模型、密钥，无需 openssl 依赖。

use std::process::Command;

// ── 极简 JSON 解析器（无 serde 依赖） ─────────────────────────────
struct JsonParser;

impl JsonParser {
    /// 从 OpenAI 格式 JSON 中提取 choices[0].message.content
    pub fn extract_content(json: &str) -> Result<String, String> {
        // 定位 "choices" 数组
        if let Some(choices_start) = json.find("\"choices\"") {
            // 找到 choices 后面的 [
            let after = &json[choices_start + 10..];
            if let Some(bracket_pos) = after.find('[') {
                let array = &after[bracket_pos + 1..];
                // 找第一个 { (第一个 choice)
                if let Some(choice_start) = array.find('{') {
                    let choice = &array[choice_start + 1..];
                    // 在第一个 choice 对象中找 "message"
                    if let Some(msg_start) = choice.find("\"message\"") {
                        let after_msg = &choice[msg_start + 10..];
                        if let Some(brace_pos) = after_msg.find('{') {
                            let msg_obj = &after_msg[brace_pos + 1..];
                            // 在 message 中找 "content"
                            if let Some(content_key) = msg_obj.find("\"content\"") {
                                let after_content = &msg_obj[content_key + 9..];
                                if !after_content.starts_with(':') { return Err("AI返回数据解析异常".into()); }
                                let after_colon = after_content[1..].trim_start();
                                if after_colon.starts_with('"') {
                                    if let Some(s) = Self::parse_string(&after_colon[1..]) { return Ok(s); }
                                }
                            }
                        }
                    }
                }
            }
        }
        // fallback: 如果找不到 choices 结构，尝试找最后一个 "content"（兼容简单响应）
        let mut search_from = 0;
        let mut last_pos = 0;
        while let Some(pos) = json[search_from..].find("\"content\"") {
            last_pos = search_from + pos;
            search_from = last_pos + 1;
        }
        if last_pos == 0 && !json.contains("\"content\"") {
            return Err("AI返回数据解析异常".into());
        }
        let after_key = json[last_pos + 9..].trim_start();
        if !after_key.starts_with(':') { return Err("AI返回数据解析异常".into()); }
        let after_colon = after_key[1..].trim_start();
        if after_colon.starts_with('"') {
            if let Some(s) = Self::parse_string(&after_colon[1..]) { return Ok(s); }
        }
        Err("AI返回数据解析异常".into())
    }

    fn parse_string(s: &str) -> Option<String> {
        let mut result = String::new();
        let mut chars = s.chars();
        let mut escaped = false;
        while let Some(ch) = chars.next() {
            if escaped {
                match ch {
                    '"' => result.push('"'), '\\' => result.push('\\'), '/' => result.push('/'),
                    'n' => result.push('\n'), 'r' => result.push('\r'), 't' => result.push('\t'),
                    'b' => result.push('\x08'), 'f' => result.push('\x0c'),
                    'u' => {
                        let hex: String = chars.by_ref().take(4).collect();
                        if hex.len() == 4 {
                            if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                                result.push(c);
                            }
                        }
                    }
                    _ => { result.push('\\'); result.push(ch); }
                }
                escaped = false;
            } else if ch == '\\' { escaped = true; }
            else if ch == '"' { return Some(result); }
            else { result.push(ch); }
        }
        None
    }

    pub fn check_error(json: &str) -> Option<String> {
        if !json.contains("\"error\"") { return None; }
        // 尝试解析 error.message
        if let Some(err_start) = json.find("\"error\"") {
            let after = &json[err_start + 7..];
            if let Some(brace_pos) = after.find('{') {
                let err_obj = &after[brace_pos + 1..];
                if let Some(msg_start) = err_obj.find("\"message\"") {
                    let after_msg = &err_obj[msg_start + 9..];
                    let after_colon = after_msg.trim_start();
                    if after_colon.starts_with(':') {
                        let val = after_colon[1..].trim_start();
                        if val.starts_with('"') {
                            if let Some(s) = Self::parse_string(&val[1..]) {
                                return Some(s);
                            }
                        }
                    }
                }
            }
        }
        Some("未知API错误".into())
    }
}

// ── 通用 API 客户端（支持任意 OpenAI 兼容提供商） ─────────────────
pub struct ApiClient {
    api_base: String,
    api_key: String,
    model_id: String,
    system_prompt: String,
}

impl ApiClient {
    pub fn new(api_base: &str, api_key: &str, model_id: &str, system_prompt: &str) -> Self {
        ApiClient {
            api_base: api_base.to_string(),
            api_key: api_key.to_string(),
            model_id: model_id.to_string(),
            system_prompt: system_prompt.to_string(),
        }
    }

    pub fn send_message(&mut self, messages: &[(String, String)]) -> Result<String, String> {
        let mut body = format!(
            r#"{{"model":"{}","temperature":1,"top_p":0.95,"max_tokens":8192,"stream":false,"messages":["#,
            self.model_id
        );
        if !self.system_prompt.is_empty() {
            body.push_str(&format!(r#"{{"role":"system","content":"{}"}},"#, escape_json(&self.system_prompt)));
        }
        for (i, (role, content)) in messages.iter().enumerate() {
            if i > 0 || !self.system_prompt.is_empty() { body.push(','); }
            let jr = if role == "user" { "user" } else { "assistant" };
            body.push_str(&format!(r#"{{"role":"{}","content":"{}"}}"#, jr, escape_json(content)));
        }
        body.push_str("]}");

        let tmp_path = format!("/tmp/kk_chat_post_{}.json", std::process::id());
        if let Err(_) = std::fs::write(tmp_path, &body) {
            return Err("写入临时请求文件失败".into());
        }

        let output = Command::new("/bin/busybox")
            .args(&["wget", "-q", "-O", "-", "--timeout=30",
                     "--post-file", &tmp_path,
                     "--header", &format!("Authorization: Bearer {}", self.api_key),
                     "--header", "Content-Type: application/json",
                     &self.api_base])
            .output()
            .map_err(|_| "网络请求失败，请检查网线以太网连接".to_string())?;

        let _ = std::fs::remove_file(tmp_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("401") || stderr.contains("403") {
                return Err("API密钥无效，请前往设置更换密钥".into());
            }
            if stderr.contains("404") || stderr.contains("400") {
                return Err("API地址或模型无效，请检查设置".into());
            }
            if stderr.contains("503") || stderr.contains("429") {
                return Err("请求超时，可稍后重试或切换其他模型".into());
            }
            return Err("网络请求失败，请检查网线以太网连接".into());
        }

        let response_body = String::from_utf8_lossy(&output.stdout);

        if let Some(err) = JsonParser::check_error(&response_body) {
            if err.contains("auth") || err.contains("key") || err.contains("API") {
                return Err("API密钥无效，请前往设置更换密钥".into());
            }
            if err.contains("model") {
                return Err("当前选择模型无效，请更换列表内可用模型".into());
            }
            return Err(format!("AI返回数据解析异常: {}", err));
        }

        JsonParser::extract_content(&response_body)
    }
}

fn escape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => result.push_str(&format!("\\u{:04x}", c as u32)),
            c => result.push(c),
        }
    }
    result
}