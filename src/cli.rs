use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::analyzer::AnalyzeOptions;
use crate::error::{AppError, AppResult};
use crate::model::{LogFilter, LogLevel, OutputFormat};

#[derive(Debug, Clone)]
pub enum Command {
    Analyze(Config),
    Demo,
    Help,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub format: OutputFormat,
    pub options: AnalyzeOptions,
}

pub fn parse_env() -> AppResult<Command> {
    parse_args(env::args().skip(1))
}

pub fn parse_args<I>(args: I) -> AppResult<Command>
where
    I: IntoIterator<Item = String>,
{
    let mut args: Vec<String> = args.into_iter().collect();
    if args.is_empty() {
        return Ok(Command::Help);
    }

    let command = args.remove(0);
    match command.as_str() {
        "help" | "--help" | "-h" => Ok(Command::Help),
        "demo" => Ok(Command::Demo),
        "analyze" | "run" => parse_analyze(args),
        other => Err(AppError::InvalidArgument(format!(
            "unknown command `{other}`; use `help` to see usage"
        ))),
    }
}

fn parse_analyze(args: Vec<String>) -> AppResult<Command> {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut format = OutputFormat::Text;
    let mut threads = 4usize;
    let mut top_n = 5usize;
    let mut slow_threshold_ms = 500u64;
    let mut min_level: Option<LogLevel> = None;
    let mut service: Option<String> = None;
    let mut keyword: Option<String> = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" | "-o" => output = Some(PathBuf::from(next_value(&mut iter, &arg)?)),
            "--format" | "-f" => format = next_value(&mut iter, &arg)?.parse()?,
            "--threads" | "-t" => {
                threads = parse_positive_usize(&next_value(&mut iter, &arg)?, "threads")?
            }
            "--top" => top_n = parse_positive_usize(&next_value(&mut iter, &arg)?, "top")?,
            "--slow" => {
                slow_threshold_ms = parse_positive_u64(&next_value(&mut iter, &arg)?, "slow")?
            }
            "--level" | "-l" => min_level = Some(next_value(&mut iter, &arg)?.parse()?),
            "--service" | "-s" => service = Some(next_value(&mut iter, &arg)?),
            "--keyword" | "-k" => keyword = Some(next_value(&mut iter, &arg)?),
            value if value.starts_with('-') => {
                return Err(AppError::InvalidArgument(format!("unknown flag `{value}`")))
            }
            value => {
                if input.is_some() {
                    return Err(AppError::InvalidArgument(format!(
                        "multiple input files are not supported; unexpected `{value}`"
                    )));
                }
                input = Some(PathBuf::from(value));
            }
        }
    }

    let input = input.ok_or_else(|| {
        AppError::InvalidArgument("missing input file: rlog analyze <FILE>".to_string())
    })?;

    Ok(Command::Analyze(Config {
        input,
        output,
        format,
        options: AnalyzeOptions {
            threads,
            top_n,
            slow_threshold_ms,
            filter: LogFilter {
                min_level,
                service,
                keyword,
            },
        },
    }))
}

fn next_value<I>(iter: &mut I, flag: &str) -> AppResult<String>
where
    I: Iterator<Item = String>,
{
    iter.next()
        .ok_or_else(|| AppError::InvalidArgument(format!("flag `{flag}` needs a value")))
}

fn parse_positive_usize(text: &str, name: &str) -> AppResult<usize> {
    let value = text.parse::<usize>()?;
    if value == 0 {
        Err(AppError::InvalidArgument(format!(
            "{name} must be positive"
        )))
    } else {
        Ok(value)
    }
}

fn parse_positive_u64(text: &str, name: &str) -> AppResult<u64> {
    let value = text.parse::<u64>()?;
    if value == 0 {
        Err(AppError::InvalidArgument(format!(
            "{name} must be positive"
        )))
    } else {
        Ok(value)
    }
}

/// Read a UTF-8 log file and split it into lines.
///
/// The command-line layer owns file input because it is part of the user
/// interaction boundary. Keeping it here makes the parser focus only on
/// turning one line of text into one structured record.
pub fn read_lines(path: &Path) -> AppResult<Vec<String>> {
    let content = fs::read_to_string(path)?;
    Ok(content.lines().map(ToString::to_string).collect())
}

/// Write the rendered report either to a file or to stdout.
pub fn write_output(path: Option<&Path>, content: &str) -> AppResult<()> {
    match path {
        Some(path) => {
            fs::write(path, content)?;
        }
        None => {
            let mut stdout = io::stdout().lock();
            stdout.write_all(content.as_bytes())?;
        }
    }
    Ok(())
}

/// Built-in demo data used by `cargo run -- demo`.
///
/// The dataset intentionally contains normal records, error records, slow
/// records and debug records, so the demo can exercise every major branch of
/// the analyzer without relying on an external file.
pub fn demo_lines() -> Vec<String> {
    vec![
        "time=2026-06-01T10:00:01 service=auth level=INFO latency=32 msg=\"login page opened\"".to_string(),
        "time=2026-06-01T10:00:02 service=auth level=ERROR latency=145 msg=\"login failed\"".to_string(),
        "time=2026-06-01T10:00:03 service=order level=WARN latency=610 msg=\"slow query\"".to_string(),
        "time=2026-06-01T10:00:04 service=payment level=ERROR latency=830 msg=\"payment timeout\"".to_string(),
        "time=2026-06-01T10:00:05 service=search level=DEBUG latency=11 msg=\"cache hit\"".to_string(),
        "time=2026-06-01T10:00:06 service=order level=INFO latency=78 msg=\"order created\"".to_string(),
        "time=2026-06-01T10:00:07 service=payment level=WARN latency=540 msg=\"retry payment gateway\"".to_string(),
    ]
}

pub fn usage() -> &'static str {
    "Rust Log Lab\n\nUSAGE:\n  cargo run -- analyze <FILE> [options]\n  cargo run -- demo\n\nOPTIONS:\n  -o, --out <FILE>       Write report to file\n  -f, --format <FORMAT>  text | json | csv, default text\n  -t, --threads <N>      Worker thread count, default 4\n      --top <N>          Number of top items to display, default 5\n      --slow <MS>        Slow request threshold, default 500\n  -l, --level <LEVEL>    Minimum level: DEBUG/INFO/WARN/ERROR/FATAL\n  -s, --service <NAME>   Keep only one service\n  -k, --keyword <TEXT>   Keep only records containing keyword\n\nLOG FORMAT:\n  service=auth level=ERROR latency=120 msg=\"login failed\"\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_analyze_command() {
        let cmd = parse_args(vec![
            "analyze".to_string(),
            "a.log".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--threads".to_string(),
            "2".to_string(),
        ])
        .unwrap();
        match cmd {
            Command::Analyze(config) => {
                assert_eq!(config.format, OutputFormat::Json);
                assert_eq!(config.options.threads, 2);
            }
            _ => panic!("wrong command"),
        }
    }

    #[test]
    fn demo_dataset_covers_multiple_levels() {
        let lines = demo_lines();
        assert!(lines.iter().any(|line| line.contains("level=ERROR")));
        assert!(lines.iter().any(|line| line.contains("level=WARN")));
        assert!(lines.iter().any(|line| line.contains("level=DEBUG")));
    }
}
