mod analyzer;
mod cli;
mod error;
mod model;
mod parser;
mod report;

use analyzer::{analyze_lines, AnalyzeOptions};
use cli::{demo_lines, parse_env, read_lines, usage, write_output, Command};
use error::AppResult;
use model::OutputFormat;
use report::render_report;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        eprintln!("\n{}", usage());
        std::process::exit(1);
    }
}

fn run() -> AppResult<()> {
    match parse_env()? {
        Command::Help => {
            println!("{}", usage());
            Ok(())
        }
        Command::Demo => {
            let options = AnalyzeOptions::default();
            let lines = demo_lines();
            let result = analyze_lines(&lines, options.clone())?;
            let output = render_report(&result, OutputFormat::Text, options.top_n);
            write_output(None, &output)
        }
        Command::Analyze(config) => {
            let lines = read_lines(&config.input)?;
            let result = analyze_lines(&lines, config.options.clone())?;
            let output = render_report(&result, config.format, config.options.top_n);
            write_output(config.output.as_deref(), &output)
        }
    }
}
