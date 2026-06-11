//! Rust Log Lab library entry.
//!
//! The binary target in `main.rs` is only a thin command-line wrapper. Most
//! business logic is placed in this library-style module tree, which makes the
//! code easier to test and reuse.

pub mod analyzer;
pub mod cli;
pub mod error;
pub mod model;
pub mod parser;
pub mod report;

pub use analyzer::{analyze_lines, AnalysisResult, AnalyzeOptions};
pub use error::{AppError, AppResult};
pub use model::{LogLevel, LogRecord, OutputFormat};

#[cfg(test)]
mod integration_smoke_tests {
    use super::*;

    #[test]
    fn public_api_can_analyze_one_line() {
        let lines = vec!["service=auth level=INFO latency=1 msg=ok".to_string()];
        let result = analyze_lines(&lines, AnalyzeOptions::default()).unwrap();
        assert_eq!(result.parsed_records, 1);
    }
}
