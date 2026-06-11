use std::collections::BTreeMap;

use crate::error::{AppError, AppResult};
use crate::model::{LogLevel, LogRecord};

/// Parser trait makes the parser replaceable and testable.
pub trait RecordParser {
    fn parse_line(&self, line_number: usize, line: &str) -> AppResult<Option<LogRecord>>;
}

#[derive(Debug, Default, Clone)]
pub struct KeyValueParser;

impl KeyValueParser {
    pub fn new() -> Self {
        Self
    }
}

impl RecordParser for KeyValueParser {
    fn parse_line(&self, line_number: usize, line: &str) -> AppResult<Option<LogRecord>> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return Ok(None);
        }

        let fields = parse_key_value_fields(trimmed)?;
        let service = fields
            .get("service")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let level = match fields.get("level") {
            Some(value) => value.parse::<LogLevel>()?,
            None => LogLevel::Unknown,
        };
        let latency_ms = parse_latency(&fields)?;
        let message = fields
            .get("msg")
            .or_else(|| fields.get("message"))
            .cloned()
            .unwrap_or_default();

        Ok(Some(LogRecord::new(
            line_number,
            service,
            level,
            latency_ms,
            message,
            fields,
            trimmed.to_string(),
        )))
    }
}

fn parse_latency(fields: &BTreeMap<String, String>) -> AppResult<Option<u64>> {
    let candidate = fields
        .get("latency")
        .or_else(|| fields.get("latency_ms"))
        .or_else(|| fields.get("cost"));
    match candidate {
        Some(text) => {
            let normalized = text
                .trim()
                .trim_end_matches("ms")
                .trim_end_matches("MS")
                .trim();
            if normalized.is_empty() {
                return Ok(None);
            }
            Ok(Some(normalized.parse::<u64>()?))
        }
        None => Ok(None),
    }
}

/// Parse key=value fields.
///
/// Supported examples:
/// - service=auth level=ERROR msg="login failed"
/// - latency=30ms service=order msg=create_order
/// - values can be wrapped in double quotes and may contain spaces
pub fn parse_key_value_fields(line: &str) -> AppResult<BTreeMap<String, String>> {
    let chars: Vec<char> = line.chars().collect();
    let mut pos = 0;
    let mut map = BTreeMap::new();

    while pos < chars.len() {
        skip_spaces(&chars, &mut pos);
        if pos >= chars.len() {
            break;
        }

        let key_start = pos;
        while pos < chars.len() && chars[pos] != '=' && !chars[pos].is_whitespace() {
            pos += 1;
        }
        if pos >= chars.len() || chars[pos] != '=' {
            return Err(AppError::Parse(format!(
                "line contains token without `=` near `{}`",
                chars[key_start..].iter().collect::<String>()
            )));
        }
        let key: String = chars[key_start..pos].iter().collect();
        if key.trim().is_empty() {
            return Err(AppError::Parse("empty key".to_string()));
        }
        pos += 1;

        let value = if pos < chars.len() && chars[pos] == '"' {
            pos += 1;
            let value_start = pos;
            let mut escaped = false;
            let mut out = String::new();
            while pos < chars.len() {
                let ch = chars[pos];
                if escaped {
                    out.push(ch);
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    break;
                } else {
                    out.push(ch);
                }
                pos += 1;
            }
            if pos >= chars.len() || chars[pos] != '"' {
                return Err(AppError::Parse(format!(
                    "unclosed quote starting at column {value_start}"
                )));
            }
            pos += 1;
            out
        } else {
            let value_start = pos;
            while pos < chars.len() && !chars[pos].is_whitespace() {
                pos += 1;
            }
            chars[value_start..pos].iter().collect()
        };

        map.insert(key.trim().to_ascii_lowercase(), value);
    }

    if map.is_empty() {
        Err(AppError::Parse("no key=value fields found".to_string()))
    } else {
        Ok(map)
    }
}

fn skip_spaces(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() && chars[*pos].is_whitespace() {
        *pos += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quoted_message() {
        let map = parse_key_value_fields("service=auth level=ERROR msg=\"login failed\"").unwrap();
        assert_eq!(map.get("service").unwrap(), "auth");
        assert_eq!(map.get("msg").unwrap(), "login failed");
    }

    #[test]
    fn parser_ignores_empty_line() {
        let parser = KeyValueParser::new();
        assert!(parser.parse_line(1, "   ").unwrap().is_none());
    }

    #[test]
    fn parser_builds_record() {
        let parser = KeyValueParser::new();
        let record = parser
            .parse_line(7, "service=pay level=WARN latency=23ms msg=retry")
            .unwrap()
            .unwrap();
        assert_eq!(record.line_number, 7);
        assert_eq!(record.service, "pay");
        assert_eq!(record.latency_ms, Some(23));
    }
}
