//! 会话导出：把一个会话的全部消息渲染成 JSON / Markdown / HTML。
//!
//! 无 Tauri 依赖，Tauri 命令与 web 路由共用。文件名由调用方（前端）决定，
//! 这里只负责把路径校验后读出全部消息并渲染成字符串。

use std::path::Path;

use serde::Serialize;

use crate::models::message::{DisplayContentBlock, DisplayMessage};
use crate::paths::validate_session_file;
use crate::provider::{claude, codex, grok};

/// 导出格式。前端以小写字符串传入（json / markdown / html）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Markdown,
    Html,
}

impl ExportFormat {
    /// 容错解析：兼容 "md" / "htm" 等常见写法。
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "markdown" | "md" => Ok(Self::Markdown),
            "html" | "htm" => Ok(Self::Html),
            other => Err(format!("Unknown export format: {}", other)),
        }
    }
}

/// 读取会话全部消息并渲染为指定格式的字符串。
pub fn render_session(
    source: &str,
    file_path: &str,
    format: ExportFormat,
) -> Result<String, String> {
    // 同删除一样，先校验路径落在数据源允许的根目录内，防止任意文件读取。
    let path = validate_session_file(source, file_path)?;

    let messages = match source {
        "claude" => claude::parse_all_messages(&path),
        "codex" => codex::parse_all_messages(&path),
        "grok" => grok::parse_all_messages(&path),
        _ => return Err(format!("Unknown source: {}", source)),
    }?;

    Ok(match format {
        ExportFormat::Json => render_json(source, &path, &messages)?,
        ExportFormat::Markdown => render_markdown(&messages),
        ExportFormat::Html => render_html(&messages),
    })
}

// ── JSON ──

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonExport<'a> {
    source: &'a str,
    file_path: String,
    message_count: usize,
    messages: &'a [DisplayMessage],
}

fn render_json(
    source: &str,
    path: &Path,
    messages: &[DisplayMessage],
) -> Result<String, String> {
    let export = JsonExport {
        source,
        file_path: path.to_string_lossy().to_string(),
        message_count: messages.len(),
        messages,
    };
    serde_json::to_string_pretty(&export).map_err(|e| format!("Failed to serialize JSON: {}", e))
}

// ── Markdown ──

fn role_heading(role: &str) -> &'static str {
    match role {
        "user" => "## 👤 User",
        "assistant" => "## 🤖 Assistant",
        "system" => "## ⚙️ System",
        _ => "## 💬 Message",
    }
}

fn render_markdown(messages: &[DisplayMessage]) -> String {
    let mut out = String::new();
    out.push_str("# 会话导出\n\n");

    for msg in messages {
        out.push_str(role_heading(&msg.role));
        if let Some(ts) = &msg.timestamp {
            out.push_str(&format!("  \n*{}*", ts));
        }
        if let Some(model) = &msg.model {
            out.push_str(&format!("  \n`{}`", model));
        }
        out.push_str("\n\n");

        for block in &msg.content {
            render_block_markdown(block, &mut out);
        }
        out.push_str("\n---\n\n");
    }

    out
}

fn fenced(lang: &str, body: &str, out: &mut String) {
    out.push_str("```");
    out.push_str(lang);
    out.push('\n');
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("```\n\n");
}

fn render_block_markdown(block: &DisplayContentBlock, out: &mut String) {
    match block {
        DisplayContentBlock::Text { text } => {
            out.push_str(text);
            out.push_str("\n\n");
        }
        DisplayContentBlock::Thinking { thinking } => {
            out.push_str("> 💭 **Thinking**\n>\n");
            for line in thinking.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }
        DisplayContentBlock::Reasoning { text } => {
            out.push_str("> 💭 **Reasoning**\n>\n");
            for line in text.lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }
        DisplayContentBlock::ToolUse { name, input, .. } => {
            out.push_str(&format!("**🔧 Tool: `{}`**\n\n", name));
            fenced("json", input, out);
        }
        DisplayContentBlock::FunctionCall { name, arguments, .. } => {
            out.push_str(&format!("**🔧 Function: `{}`**\n\n", name));
            fenced("json", arguments, out);
        }
        DisplayContentBlock::ToolResult { content, is_error, .. } => {
            out.push_str(if *is_error {
                "**❌ Tool Result (error)**\n\n"
            } else {
                "**✅ Tool Result**\n\n"
            });
            fenced("", content, out);
        }
        DisplayContentBlock::FunctionCallOutput { output, .. } => {
            out.push_str("**✅ Function Output**\n\n");
            fenced("", output, out);
        }
    }
}

// ── HTML ──

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

const HTML_STYLE: &str = r#"
:root { color-scheme: light dark; }
* { box-sizing: border-box; }
body { font-family: -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, "PingFang SC", "Microsoft YaHei", sans-serif; line-height: 1.6; margin: 0; padding: 2rem 1rem; background: #f6f7f9; color: #1f2328; }
.container { max-width: 880px; margin: 0 auto; }
h1 { font-size: 1.5rem; margin-bottom: 1.5rem; }
.msg { border: 1px solid #e3e6ea; border-radius: 10px; padding: 1rem 1.25rem; margin-bottom: 1rem; background: #fff; }
.msg.user { border-left: 4px solid #3b82f6; }
.msg.assistant { border-left: 4px solid #10b981; }
.msg.system { border-left: 4px solid #9ca3af; }
.role { font-weight: 600; margin-bottom: .5rem; }
.meta { font-size: .75rem; color: #6b7280; font-weight: 400; margin-left: .5rem; }
.text { white-space: pre-wrap; word-break: break-word; }
.thinking { background: #f1f3f5; border-radius: 6px; padding: .5rem .75rem; margin: .5rem 0; color: #4b5563; font-size: .9rem; white-space: pre-wrap; }
.tool-label { font-size: .8rem; font-weight: 600; color: #6b7280; margin: .5rem 0 .25rem; }
.tool-label.error { color: #dc2626; }
pre { background: #0d1117; color: #e6edf3; border-radius: 6px; padding: .75rem 1rem; overflow-x: auto; font-family: "JetBrains Mono", ui-monospace, SFMono-Regular, Consolas, monospace; font-size: .82rem; white-space: pre-wrap; word-break: break-word; }
@media (prefers-color-scheme: dark) {
  body { background: #0d1117; color: #e6edf3; }
  .msg { background: #161b22; border-color: #30363d; }
  .thinking { background: #21262d; color: #b0b8c0; }
}
"#;

fn render_html(messages: &[DisplayMessage]) -> String {
    let mut body = String::new();
    for msg in messages {
        let role_class = match msg.role.as_str() {
            "user" => "user",
            "assistant" => "assistant",
            "system" => "system",
            _ => "other",
        };
        body.push_str(&format!("<div class=\"msg {}\">", role_class));
        body.push_str(&format!(
            "<div class=\"role\">{}",
            escape_html(&display_role(&msg.role))
        ));
        let mut meta = String::new();
        if let Some(ts) = &msg.timestamp {
            meta.push_str(&escape_html(ts));
        }
        if let Some(model) = &msg.model {
            if !meta.is_empty() {
                meta.push_str(" · ");
            }
            meta.push_str(&escape_html(model));
        }
        if !meta.is_empty() {
            body.push_str(&format!("<span class=\"meta\">{}</span>", meta));
        }
        body.push_str("</div>");

        for block in &msg.content {
            render_block_html(block, &mut body);
        }
        body.push_str("</div>\n");
    }

    format!(
        "<!DOCTYPE html>\n<html lang=\"zh\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>会话导出</title>\n<style>{}</style>\n</head>\n<body>\n<div class=\"container\">\n<h1>会话导出</h1>\n{}</div>\n</body>\n</html>\n",
        HTML_STYLE, body
    )
}

fn display_role(role: &str) -> String {
    match role {
        "user" => "👤 User".to_string(),
        "assistant" => "🤖 Assistant".to_string(),
        "system" => "⚙️ System".to_string(),
        other => format!("💬 {}", other),
    }
}

fn pre_block(body: &str, out: &mut String) {
    out.push_str("<pre>");
    out.push_str(&escape_html(body));
    out.push_str("</pre>");
}

fn render_block_html(block: &DisplayContentBlock, out: &mut String) {
    match block {
        DisplayContentBlock::Text { text } => {
            out.push_str(&format!("<div class=\"text\">{}</div>", escape_html(text)));
        }
        DisplayContentBlock::Thinking { thinking } => {
            out.push_str(&format!(
                "<div class=\"thinking\">💭 {}</div>",
                escape_html(thinking)
            ));
        }
        DisplayContentBlock::Reasoning { text } => {
            out.push_str(&format!(
                "<div class=\"thinking\">💭 {}</div>",
                escape_html(text)
            ));
        }
        DisplayContentBlock::ToolUse { name, input, .. } => {
            out.push_str(&format!(
                "<div class=\"tool-label\">🔧 Tool: {}</div>",
                escape_html(name)
            ));
            pre_block(input, out);
        }
        DisplayContentBlock::FunctionCall { name, arguments, .. } => {
            out.push_str(&format!(
                "<div class=\"tool-label\">🔧 Function: {}</div>",
                escape_html(name)
            ));
            pre_block(arguments, out);
        }
        DisplayContentBlock::ToolResult { content, is_error, .. } => {
            out.push_str(&format!(
                "<div class=\"tool-label{}\">{}</div>",
                if *is_error { " error" } else { "" },
                if *is_error { "❌ Tool Result (error)" } else { "✅ Tool Result" }
            ));
            pre_block(content, out);
        }
        DisplayContentBlock::FunctionCallOutput { output, .. } => {
            out.push_str("<div class=\"tool-label\">✅ Function Output</div>");
            pre_block(output, out);
        }
    }
}
