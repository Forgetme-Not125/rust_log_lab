use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
    Unknown,
}

impl LogLevel {
    pub fn severity(&self) -> u8 {
        match self {
            LogLevel::Trace => 0,
            LogLevel::Debug => 1,
            LogLevel::Info => 2,
            LogLevel::Warn => 3,
            LogLevel::Error => 4,
            LogLevel::Fatal => 5,
            LogLevel::Unknown => 6,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
            LogLevel::Unknown => "UNKNOWN",
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for LogLevel {
    type Err = AppError;

    fn from_str(s: &str) -> AppResult<Self> {
        let level = match s.trim().to_ascii_uppercase().as_str() {
            "TRACE" => LogLevel::Trace,
            "DEBUG" => LogLevel::Debug,
            "INFO" => LogLevel::Info,
            "WARN" | "WARNING" => LogLevel::Warn,
            "ERROR" | "ERR" => LogLevel::Error,
            "FATAL" => LogLevel::Fatal,
            "UNKNOWN" | "" => LogLevel::Unknown,
            other => return Err(AppError::Parse(format!("unsupported log level `{other}`"))),
        };
        Ok(level)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRecord {
    pub line_number: usize,
    pub service: String,
    pub level: LogLevel,
    pub latency_ms: Option<u64>,
    pub message: String,
    pub fields: BTreeMap<String, String>,
    pub raw: String,
}

impl LogRecord {
    pub fn new(
        line_number: usize,
        service: String,
        level: LogLevel,
        latency_ms: Option<u64>,
        message: String,
        fields: BTreeMap<String, String>,
        raw: String,
    ) -> Self {
        Self {
            line_number,
            service,
            level,
            latency_ms,
            message,
            fields,
            raw,
        }
    }

    pub fn is_error_like(&self) -> bool {
        matches!(self.level, LogLevel::Error | LogLevel::Fatal)
    }

    pub fn field(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFilter {
    pub min_level: Option<LogLevel>,
    pub service: Option<String>,
    pub keyword: Option<String>,
}

impl LogFilter {
    pub fn accept(&self, record: &LogRecord) -> bool {
        if let Some(level) = self.min_level {
            if record.level.severity() < level.severity() {
                return false;
            }
        }
        if let Some(service) = &self.service {
            if !record.service.eq_ignore_ascii_case(service) {
                return false;
            }
        }
        if let Some(keyword) = &self.keyword {
            let key = keyword.to_ascii_lowercase();
            let in_message = record.message.to_ascii_lowercase().contains(&key);
            let in_raw = record.raw.to_ascii_lowercase().contains(&key);
            if !in_message && !in_raw {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Markdown,
    Json,
    Csv,
}

impl FromStr for OutputFormat {
    type Err = AppError;

    fn from_str(s: &str) -> AppResult<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "text" | "txt" => Ok(OutputFormat::Text),
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            "json" => Ok(OutputFormat::Json),
            "csv" => Ok(OutputFormat::Csv),
            other => Err(AppError::InvalidArgument(format!(
                "unknown format `{other}`, expected text/markdown/json/csv"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TopItem<K> {
    pub key: K,
    pub count: usize,
}

impl<K: Ord> Ord for TopItem<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.count
            .cmp(&other.count)
            .then_with(|| other.key.cmp(&self.key))
    }
}

impl<K: Ord> PartialOrd for TopItem<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: Ord> PartialEq for TopItem<K> {
    fn eq(&self, other: &Self) -> bool {
        self.count == other.count && self.key == other.key
    }
}

impl<K: Ord> Eq for TopItem<K> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_level_is_case_insensitive() {
        assert_eq!("warn".parse::<LogLevel>().unwrap(), LogLevel::Warn);
        assert_eq!("ERROR".parse::<LogLevel>().unwrap(), LogLevel::Error);
    }

    #[test]
    fn filter_checks_min_level_and_service() {
        let record = LogRecord::new(
            1,
            "auth".to_string(),
            LogLevel::Error,
            Some(42),
            "bad".to_string(),
            BTreeMap::new(),
            "raw".to_string(),
        );
        let filter = LogFilter {
            min_level: Some(LogLevel::Warn),
            service: Some("AUTH".to_string()),
            keyword: None,
        };
        assert!(filter.accept(&record));
    }
}
