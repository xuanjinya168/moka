//! 按配置文件类型分别执行语法检查与美化格式化：
//! - `nginx`   -> Nginx 块级语法 (`{ ... }` + `;` 分号校验 + 4 空格缩进)
//! - `mysql`   -> INI (`[section]` + `key=value`)
//! - `redis`   -> Redis 指令格式 (引号匹配 + 常用数值参数校验 + 单空格规范)
//! - `nacos`   -> Java `.properties` (`key=value` / `key: value`)
//! - `postgres`-> PostgreSQL `key = value` (单引号匹配 + 行尾注释保留)
//! - `tomcat`  -> XML 标签栈校验 + 2 空格缩进美化
//! - `mongodb` -> YAML 缩进/Tab 禁用检查 + 层级保留规范化
//! - `mailpit` -> `.env` (`KEY=VALUE`)

pub fn config_format_label(config_kind: Option<&str>) -> &'static str {
    match config_kind {
        Some("nginx") => "Nginx",
        Some("mysql") => "INI",
        Some("redis") => "Redis",
        Some("nacos") => "Properties",
        Some("postgres") => "PostgreSQL",
        Some("tomcat") => "XML",
        Some("mongodb") => "YAML",
        Some("mailpit") => "ENV",
        _ => "文本",
    }
}

/// 按配置类型先做语法检查，通过后返回格式化文本；有语法错误返回带行号的中文错误说明。
pub fn format_and_validate_config(
    config_kind: Option<&str>,
    content: &str,
) -> Result<String, String> {
    match config_kind {
        Some("nginx") => format_nginx(content),
        Some("mysql") => format_ini(content),
        Some("redis") => format_redis(content),
        Some("nacos") => format_properties(content),
        Some("postgres") => format_postgres(content),
        Some("tomcat") => format_xml(content),
        Some("mongodb") => format_yaml(content),
        Some("mailpit") => format_env(content),
        _ => Ok(collapse_blank_lines(
            &content
                .lines()
                .map(|l| l.trim_end().to_string())
                .collect::<Vec<_>>(),
        )),
    }
}

fn collapse_blank_lines(lines: &[String]) -> String {
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut prev_blank = true; // 去掉开头多余空行
    for line in lines {
        let is_blank = line.trim().is_empty();
        if is_blank {
            if !prev_blank {
                out.push(String::new());
            }
        } else {
            out.push(line.clone());
        }
        prev_blank = is_blank;
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    let mut s = out.join("\n");
    if !s.is_empty() {
        s.push('\n');
    }
    s
}

/// 检查一行中的引号是否成对闭合（忽略 `#` 之后的注释）
fn check_quotes_balanced(line: &str, comment_chars: &[char], line_no: usize) -> Result<(), String> {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for ch in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
        } else if comment_chars.contains(&ch) {
            break;
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
        }
    }
    if let Some(q) = quote {
        return Err(format!("第 {line_no} 行存在未闭合的引号 {q}"));
    }
    Ok(())
}

/// 剥离行尾注释（尊重引号内的 `#`）
fn strip_comment_respecting_quotes(line: &str, comment_char: char) -> (&str, &str) {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
        } else if ch == comment_char {
            return (&line[..idx], &line[idx..]);
        }
    }
    (line, "")
}

// ============================================================================
// 1. Nginx (`nginx.conf`)
// ============================================================================
fn format_nginx(content: &str) -> Result<String, String> {
    let mut depth: i32 = 0;
    let mut formatted: Vec<String> = Vec::new();
    let mut in_multiline_directive = false;

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let t = raw.trim();
        if t.is_empty() {
            formatted.push(String::new());
            continue;
        }
        if t.starts_with('#') {
            let indent = "    ".repeat(depth.max(0) as usize);
            formatted.push(format!("{indent}{t}"));
            continue;
        }

        check_quotes_balanced(t, &['#'], line_no)?;
        let (code_part, _) = strip_comment_respecting_quotes(t, '#');
        let code = code_part.trim();

        // 计算行首闭合 `}` 数量以决定当前行缩进
        let leading_close = code.chars().take_while(|&c| c == '}').count() as i32;
        let line_depth = depth - leading_close;
        if line_depth < 0 {
            return Err(format!("第 {line_no} 行出现多余的闭合大括号 '}}'"));
        }

        // 统计整行 `{` 与 `}`（忽略引号内）
        let mut quote: Option<char> = None;
        let mut escaped = false;
        let mut opens = 0i32;
        let mut closes = 0i32;
        for ch in code.chars() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' && quote.is_some() {
                escaped = true;
                continue;
            }
            if let Some(q) = quote {
                if ch == q {
                    quote = None;
                }
            } else if ch == '"' || ch == '\'' {
                quote = Some(ch);
            } else if ch == '{' {
                opens += 1;
            } else if ch == '}' {
                closes += 1;
            }
        }

        let next_depth = depth + opens - closes;
        if next_depth < 0 {
            return Err(format!("第 {line_no} 行大括号不匹配（'}}' 多于 '{{'）"));
        }

        // 检查单行指令结尾：允许以 `{`、`}`、`;` 结尾，或以 `\` / `'` / `"` 续行（如 log_format）
        let ends_ok = code.ends_with('{')
            || code.ends_with('}')
            || code.ends_with(';')
            || code.ends_with('\\');
        if !ends_ok {
            // 允许类似 log_format 多行字符串延续，但若下一行是普通指令或闭合括号则报错
            let next_non_empty = content
                .lines()
                .skip(idx + 1)
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with('#'));
            let looks_like_continuation = code.starts_with("log_format")
                || in_multiline_directive
                || next_non_empty.is_some_and(|nl| nl.starts_with('\'') || nl.starts_with('"'));
            if looks_like_continuation {
                in_multiline_directive = true;
            } else {
                return Err(format!(
                    "第 {line_no} 行 Nginx 指令缺少结尾分号 ';' 或 '{{'：{code}"
                ));
            }
        } else {
            in_multiline_directive = false;
        }

        let indent = "    ".repeat(line_depth as usize);
        formatted.push(format!("{indent}{t}"));
        depth = next_depth;
    }

    if depth != 0 {
        return Err(format!("Nginx 配置存在 {depth} 个未闭合的 '{{'"));
    }

    Ok(collapse_blank_lines(&formatted))
}

// ============================================================================
// 2. INI (`my.ini`)
// ============================================================================
fn format_ini(content: &str) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let t = raw.trim();
        if t.is_empty() {
            out.push(String::new());
            continue;
        }
        if t.starts_with('#') || t.starts_with(';') {
            out.push(t.to_string());
            continue;
        }
        if t.starts_with('[') {
            if !t.ends_with(']') || t.len() <= 2 {
                return Err(format!("第 {line_no} 行 INI 分段标题格式错误：{t}"));
            }
            if !out.is_empty() && !out.last().is_some_and(|l| l.is_empty()) {
                out.push(String::new());
            }
            let sec = t[1..t.len() - 1].trim();
            if sec.is_empty() {
                return Err(format!("第 {line_no} 行 INI 分段名称不能为空"));
            }
            out.push(format!("[{sec}]"));
            continue;
        }
        check_quotes_balanced(t, &['#', ';'], line_no)?;
        if let Some((k, v)) = t.split_once('=') {
            let key = k.trim();
            if key.is_empty() {
                return Err(format!("第 {line_no} 行缺少配置项名称：{t}"));
            }
            out.push(format!("{key}={}", v.trim()));
        } else {
            // MySQL 允许无值布尔标志（如 skip-name-resolve），但不允许包含空格的非法句子
            if t.contains(' ') {
                return Err(format!("第 {line_no} 行缺少 '=' 赋值符号：{t}"));
            }
            out.push(t.to_string());
        }
    }
    Ok(collapse_blank_lines(&out))
}

// ============================================================================
// 3. Redis (`redis.conf`)
// ============================================================================
fn format_redis(content: &str) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let t = raw.trim();
        if t.is_empty() {
            out.push(String::new());
            continue;
        }
        if t.starts_with('#') {
            out.push(t.to_string());
            continue;
        }
        check_quotes_balanced(t, &['#'], line_no)?;
        let (code_part, comment_part) = strip_comment_respecting_quotes(t, '#');
        let normalized = normalize_spaces_outside_quotes(code_part.trim());
        let mut parts = normalized.split_whitespace();
        if let Some(directive) = parts.next() {
            if matches!(
                directive.to_lowercase().as_str(),
                "port" | "databases" | "timeout" | "tcp-keepalive"
            ) {
                match parts.next() {
                    Some(v) if v.parse::<u64>().is_ok() => {}
                    _ => {
                        return Err(format!(
                            "第 {line_no} 行 Redis 参数 '{directive}' 需要合法的非负整数"
                        ));
                    }
                }
            }
        }
        if comment_part.is_empty() {
            out.push(normalized);
        } else {
            out.push(format!("{normalized} {}", comment_part.trim()));
        }
    }
    Ok(collapse_blank_lines(&out))
}

fn normalize_spaces_outside_quotes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut prev_space = false;
    for ch in s.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
            prev_space = false;
            continue;
        }
        if ch == '\\' && quote.is_some() {
            out.push(ch);
            escaped = true;
            prev_space = false;
            continue;
        }
        if let Some(q) = quote {
            out.push(ch);
            if ch == q {
                quote = None;
            }
            prev_space = false;
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
            out.push(ch);
            prev_space = false;
        } else if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

// ============================================================================
// 4. Java Properties (`application.properties`)
// ============================================================================
fn format_properties(content: &str) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    let mut prev_continued = false;
    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let t = raw.trim();
        if t.is_empty() {
            prev_continued = false;
            out.push(String::new());
            continue;
        }
        if t.starts_with('#') || t.starts_with('!') {
            prev_continued = false;
            out.push(t.to_string());
            continue;
        }
        if prev_continued {
            out.push(format!("  {t}"));
            prev_continued = t.ends_with('\\');
            continue;
        }
        let split_pos = t.find('=').or_else(|| t.find(':'));
        let Some(pos) = split_pos else {
            return Err(format!("第 {line_no} 行 Properties 配置缺少 '=' 或 ':'：{t}"));
        };
        let key = t[..pos].trim();
        let val = t[pos + 1..].trim();
        if key.is_empty() {
            return Err(format!("第 {line_no} 行配置键不能为空：{t}"));
        }
        prev_continued = val.ends_with('\\');
        out.push(format!("{key}={val}"));
    }
    Ok(collapse_blank_lines(&out))
}

// ============================================================================
// 5. PostgreSQL (`postgresql.conf`)
// ============================================================================
fn format_postgres(content: &str) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let t = raw.trim();
        if t.is_empty() {
            out.push(String::new());
            continue;
        }
        if t.starts_with('#') {
            out.push(t.to_string());
            continue;
        }
        check_quotes_balanced(t, &['#'], line_no)?;
        let (code, comment) = strip_comment_respecting_quotes(t, '#');
        let code = code.trim();
        let Some((k, v)) = code.split_once('=') else {
            return Err(format!("第 {line_no} 行 PostgreSQL 配置缺少 '='：{code}"));
        };
        let key = k.trim();
        let val = v.trim();
        if key.is_empty() || val.is_empty() {
            return Err(format!("第 {line_no} 行键值不完整：{code}"));
        }
        if comment.is_empty() {
            out.push(format!("{key} = {val}"));
        } else {
            out.push(format!("{key} = {val}  {}", comment.trim()));
        }
    }
    Ok(collapse_blank_lines(&out))
}

// ============================================================================
// 6. XML (`server.xml`)
// ============================================================================
enum XmlToken {
    Decl(String),
    Comment(String),
    Open { name: String, raw: String, line: usize },
    SelfClose(String),
    Close { name: String, line: usize },
    Text(String),
}

fn tokenize_xml(content: &str) -> Result<Vec<XmlToken>, String> {
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut tokens = Vec::new();

    while i < len {
        if bytes[i] == b'<' {
            let start_line = line;
            if content[i..].starts_with("<!--") {
                let Some(end_rel) = content[i + 4..].find("-->") else {
                    return Err(format!("第 {start_line} 行 XML 注释 '<!--' 未闭合"));
                };
                let end = i + 4 + end_rel + 3;
                let slice = &content[i..end];
                line += slice.bytes().filter(|&b| b == b'\n').count();
                tokens.push(XmlToken::Comment(slice.trim().to_string()));
                i = end;
                continue;
            }
            if content[i..].starts_with("<?") {
                let Some(end_rel) = content[i + 2..].find("?>") else {
                    return Err(format!("第 {start_line} 行 XML 处理指令 '<?' 未闭合"));
                };
                let end = i + 2 + end_rel + 2;
                let slice = &content[i..end];
                line += slice.bytes().filter(|&b| b == b'\n').count();
                tokens.push(XmlToken::Decl(slice.trim().to_string()));
                i = end;
                continue;
            }
            if content[i..].starts_with("<![CDATA[") {
                let Some(end_rel) = content[i + 9..].find("]]>") else {
                    return Err(format!("第 {start_line} 行 XML CDATA 未闭合"));
                };
                let end = i + 9 + end_rel + 3;
                let slice = &content[i..end];
                line += slice.bytes().filter(|&b| b == b'\n').count();
                tokens.push(XmlToken::Text(slice.to_string()));
                i = end;
                continue;
            }

            // 普通标签：扫描至引号外的 '>'
            let mut j = i + 1;
            let mut quote: Option<u8> = None;
            let mut found_gt = false;
            while j < len {
                let b = bytes[j];
                if b == b'\n' {
                    line += 1;
                }
                if let Some(q) = quote {
                    if b == q {
                        quote = None;
                    }
                } else if b == b'"' || b == b'\'' {
                    quote = Some(b);
                } else if b == b'>' {
                    found_gt = true;
                    break;
                }
                j += 1;
            }
            if !found_gt {
                return Err(format!("第 {start_line} 行 XML 标签 '<' 未找到闭合 '>'"));
            }
            if let Some(q) = quote {
                return Err(format!(
                    "第 {start_line} 行 XML 标签属性存在未闭合的引号 {}",
                    q as char
                ));
            }
            let raw_tag = &content[i..=j];
            let inner = raw_tag[1..raw_tag.len() - 1].trim();
            if inner.is_empty() {
                return Err(format!("第 {start_line} 行存在空的 XML 标签 '<>'"));
            }
            if let Some(rest) = inner.strip_prefix('/') {
                let name = rest.trim().to_string();
                if name.is_empty() {
                    return Err(format!("第 {start_line} 行闭合标签名称为空"));
                }
                tokens.push(XmlToken::Close {
                    name,
                    line: start_line,
                });
            } else if inner.ends_with('/') {
                let normalized = normalize_xml_tag(raw_tag);
                tokens.push(XmlToken::SelfClose(normalized));
            } else {
                let name = inner
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or("")
                    .to_string();
                if name.is_empty() {
                    return Err(format!("第 {start_line} 行 XML 标签名称非法：{raw_tag}"));
                }
                let normalized = normalize_xml_tag(raw_tag);
                tokens.push(XmlToken::Open {
                    name,
                    raw: normalized,
                    line: start_line,
                });
            }
            i = j + 1;
        } else {
            let start = i;
            while i < len && bytes[i] != b'<' {
                if bytes[i] == b'\n' {
                    line += 1;
                }
                i += 1;
            }
            let text = content[start..i].trim();
            if !text.is_empty() {
                tokens.push(XmlToken::Text(text.to_string()));
            }
        }
    }
    Ok(tokens)
}

fn normalize_xml_tag(raw: &str) -> String {
    normalize_spaces_outside_quotes(raw)
}

fn format_xml(content: &str) -> Result<String, String> {
    let tokens = tokenize_xml(content)?;
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    let mut idx = 0usize;

    while idx < tokens.len() {
        match &tokens[idx] {
            XmlToken::Decl(s) | XmlToken::Comment(s) => {
                let indent = "  ".repeat(stack.len());
                out.push(format!("{indent}{s}"));
                idx += 1;
            }
            XmlToken::SelfClose(s) => {
                let indent = "  ".repeat(stack.len());
                out.push(format!("{indent}{s}"));
                idx += 1;
            }
            XmlToken::Open { name, raw, line } => {
                // 检查是否是紧凑单行元素 `<Tag>text</Tag>`
                if idx + 2 < tokens.len() {
                    if let (XmlToken::Text(txt), XmlToken::Close { name: cname, .. }) =
                        (&tokens[idx + 1], &tokens[idx + 2])
                    {
                        if cname == name && !txt.contains('\n') {
                            let indent = "  ".repeat(stack.len());
                            out.push(format!("{indent}{raw}{txt}</{cname}>"));
                            idx += 3;
                            continue;
                        }
                    }
                }
                let indent = "  ".repeat(stack.len());
                out.push(format!("{indent}{raw}"));
                stack.push((name.clone(), *line));
                idx += 1;
            }
            XmlToken::Close { name, line } => {
                let Some((open_name, open_line)) = stack.pop() else {
                    return Err(format!("第 {line} 行出现多余的闭合标签 '</{name}>'"));
                };
                if open_name != *name {
                    return Err(format!(
                        "第 {line} 行 XML 标签不匹配：期望 '</{open_name}>'（打开于第 {open_line} 行），实际为 '</{name}>'"
                    ));
                }
                let indent = "  ".repeat(stack.len());
                out.push(format!("{indent}</{name}>"));
                idx += 1;
            }
            XmlToken::Text(txt) => {
                let indent = "  ".repeat(stack.len());
                out.push(format!("{indent}{txt}"));
                idx += 1;
            }
        }
    }

    if let Some((open_name, open_line)) = stack.pop() {
        return Err(format!(
            "XML 标签 '<{open_name}>'（第 {open_line} 行）未闭合"
        ));
    }

    Ok(collapse_blank_lines(&out))
}

// ============================================================================
// 7. YAML (`mongod.cfg`)
// ============================================================================
fn format_yaml(content: &str) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    // 用栈记录原始缩进层级，映射为干净的 2 空格倍增缩进，严格保留语义层级
    let mut indent_stack: Vec<usize> = vec![0];

    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            out.push(String::new());
            continue;
        }
        let leading_ws = &raw[..raw.len() - raw.trim_start().len()];
        if leading_ws.contains('\t') {
            return Err(format!(
                "第 {line_no} 行 YAML 缩进不能使用 Tab 制表符，请改用空格"
            ));
        }

        let raw_indent = leading_ws.len();
        while indent_stack.len() > 1 && *indent_stack.last().unwrap() > raw_indent {
            indent_stack.pop();
        }
        if raw_indent > *indent_stack.last().unwrap() {
            indent_stack.push(raw_indent);
        }
        let level = indent_stack.len().saturating_sub(1);
        let norm_indent = "  ".repeat(level);

        if trimmed.starts_with('#') {
            out.push(format!("{norm_indent}{trimmed}"));
            continue;
        }

        check_quotes_balanced(trimmed, &['#'], line_no)?;

        if trimmed.starts_with("- ") || trimmed == "-" {
            out.push(format!("{norm_indent}{trimmed}"));
            continue;
        }

        let (code, comment) = strip_comment_respecting_quotes(trimmed, '#');
        let code = code.trim();
        let Some((k, v)) = code.split_once(':') else {
            return Err(format!("第 {line_no} 行 YAML 语法错误，缺少 ':'：{code}"));
        };
        let key = k.trim();
        let val = v.trim();
        if key.is_empty() {
            return Err(format!("第 {line_no} 行 YAML 键名不能为空：{code}"));
        }
        let formatted_line = if val.is_empty() {
            format!("{norm_indent}{key}:")
        } else {
            format!("{norm_indent}{key}: {val}")
        };
        if comment.is_empty() {
            out.push(formatted_line);
        } else {
            out.push(format!("{formatted_line}  {}", comment.trim()));
        }
    }

    Ok(collapse_blank_lines(&out))
}

// ============================================================================
// 8. ENV (`mailpit.env`)
// ============================================================================
fn format_env(content: &str) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    for (idx, raw) in content.lines().enumerate() {
        let line_no = idx + 1;
        let t = raw.trim();
        if t.is_empty() {
            out.push(String::new());
            continue;
        }
        if t.starts_with('#') {
            out.push(t.to_string());
            continue;
        }
        check_quotes_balanced(t, &['#'], line_no)?;
        let Some((k, v)) = t.split_once('=') else {
            return Err(format!("第 {line_no} 行环境变量格式错误，缺少 '='：{t}"));
        };
        let key = k.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(format!("第 {line_no} 行环境变量名非法：'{key}'"));
        }
        out.push(format!("{key}={}", v.trim()));
    }
    Ok(collapse_blank_lines(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nginx_format_and_error_check() {
        let messy = "worker_processes 1;\nevents{\nworker_connections 1024;\n}\nhttp {\nserver {\nlisten 80;\n}\n}";
        let ok = format_and_validate_config(Some("nginx"), messy).unwrap();
        assert!(ok.contains("    worker_connections 1024;"));
        assert!(ok.contains("        listen 80;"));

        let bad_semi = "http {\n  listen 80\n}\n";
        assert!(format_and_validate_config(Some("nginx"), bad_semi).is_err());

        let bad_brace = "http {\n  server {\n    listen 80;\n}\n";
        assert!(format_and_validate_config(Some("nginx"), bad_brace).is_err());
    }

    #[test]
    fn xml_format_and_error_check() {
        let messy = "<?xml version=\"1.0\"?><Server port=\"8005\"><Service name=\"Catalina\"><Connector port=\"8080\"/></Service></Server>";
        let ok = format_and_validate_config(Some("tomcat"), messy).unwrap();
        assert!(ok.contains("  <Service name=\"Catalina\">"));
        assert!(ok.contains("    <Connector port=\"8080\"/>"));

        let bad = "<Server><Service></Server>";
        let err = format_and_validate_config(Some("tomcat"), bad).unwrap_err();
        assert!(err.contains("Service"));
    }

    #[test]
    fn yaml_format_and_tab_rejection() {
        let messy = "storage:\n    dbPath: \"C:/data\"\nnet:\n    port:   27017\n";
        let ok = format_and_validate_config(Some("mongodb"), messy).unwrap();
        assert!(ok.contains("storage:\n  dbPath: \"C:/data\"\nnet:\n  port: 27017"));

        let bad_tab = "storage:\n\tdbPath: \"C:/data\"\n";
        assert!(format_and_validate_config(Some("mongodb"), bad_tab).is_err());
    }

    #[test]
    fn ini_redis_props_pg_env_checks() {
        assert!(format_and_validate_config(Some("mysql"), "[mysqld]\nport = 3306\n").is_ok());
        assert!(format_and_validate_config(Some("mysql"), "[mysqld\nport=3306\n").is_err());

        assert!(format_and_validate_config(Some("redis"), "port    6379\n").unwrap().contains("port 6379"));
        assert!(format_and_validate_config(Some("redis"), "port abc\n").is_err());

        assert!(format_and_validate_config(Some("nacos"), "server.port = 8848\n").unwrap().contains("server.port=8848"));
        assert!(format_and_validate_config(Some("nacos"), "invalid_line_without_sep\n").is_err());

        assert!(format_and_validate_config(Some("postgres"), "port=5432 # pg\n").unwrap().contains("port = 5432  # pg"));
        assert!(format_and_validate_config(Some("postgres"), "timezone = 'Asia/Shanghai\n").is_err());

        assert!(format_and_validate_config(Some("mailpit"), "MP_UI_BIND_ADDR = 127.0.0.1:8025\n").is_ok());
        assert!(format_and_validate_config(Some("mailpit"), "BAD KEY=123\n").is_err());
    }
}
