//! HTTPS 网络请求模块
//!
//! 通过 busybox 内置 wget 发起 HTTPS 请求到任意 OpenAI 兼容 API。
//! 支持自定义 API 地址、模型、密钥，无需 openssl 依赖。

use std::process::Command;

// ── 极简 JSON 解析器（无 serde 依赖） ─────────────────────────────
struct JsonParser;

impl JsonParser {
    /// 从 OpenAI 格式 JSON 中提取 choices[0].message.content
    ///
    /// OpenAI 成功格式: {"choices": [{"message": {"role": "assistant", "content": "..."}}]}
    /// 这里取最后一个 "content" 键（兼容多 choices 场景）。
    pub fn extract_content(json: &str) -> Result<String, String> {
        // 先尝试在 choices 数组里找 content（标准 OpenAI 格式）
        if let Some(needle) = json.find("\"choices\"") {
            let tail = &json[needle..];
            // 只在 choices 之后找第一个 "content"
            if let Some(pos) = tail.find("\"content\"") {
                return Self::extract_string_value(&tail[pos..])
                    .ok_or_else(|| "AI返回数据解析异常".to_string());
            }
        }
        // 兜底：找任意一个 "content"（部分非标 API 会用）
        if let Some(pos) = json.find("\"content\"") {
            return Self::extract_string_value(&json[pos..])
                .ok_or_else(|| "AI返回数据解析异常".to_string());
        }
        Err("AI返回数据解析异常".into())
    }

    /// 从错误响应里提取 message 或 error 字段。
    /// 支持的格式：
    ///   {"error": "Invalid API key"}
    ///   {"error": {"message": "Invalid API key", "type": "..."}}
    ///   {"error": {"code": "...", "message": "..."}}
    ///   {"message": "..."}
    pub fn extract_error(json: &str) -> Option<String> {
        // 1. 优先在 "error" 键里找
        if let Some(pos) = json.find("\"error\"") {
            let tail = &json[pos..];
            // 1a. error 是对象：找嵌套 "message" 或 "code"
            if let Some(brace_pos) = tail.find('{') {
                let obj_start = brace_pos;
                // 在 error 对象内找 message / code
                if let Some(msg_pos) = tail[obj_start..].find("\"message\"") {
                    if let Some(s) = Self::extract_string_value(&tail[obj_start + msg_pos..]) {
                        if !s.is_empty() { return Some(s); }
                    }
                }
                if let Some(code_pos) = tail[obj_start..].find("\"code\"") {
                    if let Some(s) = Self::extract_string_value(&tail[obj_start + code_pos..]) {
                        if !s.is_empty() { return Some(s); }
                    }
                }
            }
            // 1b. error 是字符串
            if let Some(s) = Self::extract_string_value(tail) {
                if !s.is_empty() { return Some(s); }
            }
        }
        // 2. 顶层 "message" 字段
        if let Some(pos) = json.find("\"message\"") {
            if let Some(s) = Self::extract_string_value(&json[pos..]) {
                if !s.is_empty() { return Some(s); }
            }
        }
        None
    }

    /// 从 `"key": "..."` 这样的片段里取出 "..." 里的字符串值。
    /// 输入以 `"key"` 开头（pos 指向 "key"），要求紧跟 `: "value"`。
    fn extract_string_value(fragment: &str) -> Option<String> {
        // 跳过 "key"，找第一个未转义的 :
        let mut chars = fragment.chars().peekable();
        let mut in_str = false;
        let mut escape = false;
        while let Some(c) = chars.next() {
            if escape { escape = false; continue; }
            if c == '\\' && in_str { escape = true; continue; }
            if c == '"' { in_str = !in_str; continue; }
            if !in_str && c == ':' {
                // 跳过空白
                let rest: String = chars.collect();
                let trimmed = rest.trim_start();
                if trimmed.starts_with('"') {
                    return Self::parse_string(&trimmed[1..]);
                }
                return None;
            }
        }
        None
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
        if json.contains("\"error\"") || json.contains("\"message\"") {
            Self::extract_error(json).or_else(|| Some("未知API错误".into()))
        } else { None }
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
        if let Err(_) = std::fs::write(&tmp_path, &body) {
            return Err("写入临时请求文件失败".into());
        }

        // 调用 busybox wget 发 HTTPS 请求
        // 关键参数：
        //   -q            : 静默（不打印进度条到 stdout 干扰 JSON）
        //   -O -          : body 写到 stdout
        //   --timeout=30  : 整体超时 30 秒，防止 API 卡死时永久 hang
        //   --tries=1     : 不重试，wget 默认 20 次会让人等很久
        //   --no-check-certificate : 自签证书场景（多数用户场景）
        let output = Command::new("/bin/busybox")
            .args(&["wget", "-q", "-O", "-",
                     "--timeout=30", "--tries=1",
                     "--no-check-certificate",
                     "--post-file", &tmp_path,
                     "--header", &format!("Authorization: Bearer {}", self.api_key),
                     "--header", "Content-Type: application/json",
                     &self.api_base])
            .output()
            .map_err(|_| "网络请求失败，请检查网线以太网连接".to_string())?;

        let _ = std::fs::remove_file(&tmp_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            // busybox wget 把 HTTP 状态码打到 stderr，4xx/5xx 也是非 0 退码
            // 优先用 stdout body 解析错误（更具体）
            if !stdout.is_empty() {
                if let Some(err) = JsonParser::extract_error(&stdout) {
                    let lower = err.to_lowercase();
                    if lower.contains("auth") || lower.contains("key") || lower.contains("apikey") || lower.contains("unauthorized") {
                        return Err(format!("API密钥无效: {}", err));
                    }
                    if lower.contains("model") || lower.contains("not found") {
                        return Err(format!("模型或地址无效: {}", err));
                    }
                    if lower.contains("rate") || lower.contains("limit") || lower.contains("quota") {
                        return Err(format!("API 限流: {}", err));
                    }
                    return Err(format!("API 错误: {}", err));
                }
            }
            // 兜底按 stderr 关键字映射
            if stderr.contains("401") || stderr.contains("403") {
                return Err("API密钥无效（401/403），请前往设置更换密钥".into());
            }
            if stderr.contains("404") || stderr.contains("400") {
                return Err("API地址或模型无效（404/400），请检查设置".into());
            }
            if stderr.contains("503") || stderr.contains("429") {
                return Err("请求被限流或服务不可用（503/429），可稍后重试".into());
            }
            if stderr.contains("timed out") || stderr.contains("timeout") {
                return Err("请求超时（30s），可稍后重试或切换其他模型".into());
            }
            if stderr.contains("Unable to resolve") || stderr.contains("Name or service not known") {
                return Err("DNS 解析失败，请检查网络".into());
            }
            if stderr.contains("Connection refused") || stderr.contains("Network is unreachable") {
                return Err("网络不可达，请检查网线或 WiFi".into());
            }
            return Err("网络请求失败，请检查网络连接".into());
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