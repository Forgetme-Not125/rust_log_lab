use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::error::AppResult;
use crate::model::OutputFormat;

/// Report export subsystem.
///
/// The basic version of the project only printed the report to the terminal.
/// This module is the iteration module: it saves rendered reports to local
/// files and can infer the report format from the output file extension.
/// Keeping export logic here makes `cli.rs` focus on argument parsing and keeps
/// `report.rs` focus on rendering report content.
pub trait ReportExporter {
    fn export(&self, content: &str) -> AppResult<()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StdoutExporter;

impl ReportExporter for StdoutExporter {
    fn export(&self, content: &str) -> AppResult<()> {
        let mut stdout = io::stdout().lock();
        stdout.write_all(content.as_bytes())?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FileExporter {
    path: PathBuf,
}

impl FileExporter {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl ReportExporter for FileExporter {
    fn export(&self, content: &str) -> AppResult<()> {
        ensure_parent_dir(self.path())?;
        fs::write(self.path(), content)?;
        Ok(())
    }
}

/// Export report content to stdout or to a file.
pub fn export_report(path: Option<&Path>, content: &str) -> AppResult<()> {
    match path {
        Some(path) => FileExporter::new(path).export(content),
        None => StdoutExporter.export(content),
    }
}

/// Infer output format from file extension.
///
/// Examples:
/// - `report.md` -> Markdown
/// - `report.json` -> Json
/// - `report.csv` -> Csv
/// - unknown extension -> None, so the caller can keep its default format.
pub fn infer_format_from_path(path: &Path) -> Option<OutputFormat> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match ext.as_str() {
        "md" | "markdown" => Some(OutputFormat::Markdown),
        "json" => Some(OutputFormat::Json),
        "csv" => Some(OutputFormat::Csv),
        "txt" | "text" => Some(OutputFormat::Text),
        _ => None,
    }
}

fn ensure_parent_dir(path: &Path) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_markdown_format_from_extension() {
        assert_eq!(
            infer_format_from_path(Path::new("target/report.md")),
            Some(OutputFormat::Markdown)
        );
        assert_eq!(
            infer_format_from_path(Path::new("target/report.csv")),
            Some(OutputFormat::Csv)
        );
    }

    #[test]
    fn export_report_writes_file() {
        let path = std::env::temp_dir().join("rust_log_lab_export_test.txt");
        export_report(Some(&path), "hello export").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello export");
        let _ = fs::remove_file(path);
    }
}
