use std::fmt;

use crate::analyzer::AnalysisResult;
use crate::model::{LogLevel, OutputFormat};

// -----------------------------------------------------------------------------
// Report insight subsystem
// -----------------------------------------------------------------------------
// Health-score rules are kept in `report.rs` instead of a tiny separate module.
// For a single-person final assignment, this keeps the module tree compact while
// preserving the same functionality: report rendering still has a dedicated
// trait, and the rule-based health analysis is still easy to test and extend.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthLevel {
    Healthy,
    Notice,
    Risky,
    Critical,
}

impl HealthLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            HealthLevel::Healthy => "HEALTHY",
            HealthLevel::Notice => "NOTICE",
            HealthLevel::Risky => "RISKY",
            HealthLevel::Critical => "CRITICAL",
        }
    }
}

impl fmt::Display for HealthLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct Insight {
    pub health: HealthLevel,
    pub score: u8,
    pub error_ratio: f64,
    pub slow_ratio: f64,
    pub dominant_service: Option<String>,
    pub suggestions: Vec<String>,
}

impl Insight {
    pub fn empty() -> Self {
        Self {
            health: HealthLevel::Healthy,
            score: 100,
            error_ratio: 0.0,
            slow_ratio: 0.0,
            dominant_service: None,
            suggestions: vec!["没有可分析记录，请检查输入或过滤条件。".to_string()],
        }
    }
}

pub trait Rule {
    fn apply(&self, result: &AnalysisResult, draft: &mut InsightDraft);
}

#[derive(Debug, Default)]
pub struct InsightDraft {
    penalty: u8,
    suggestions: Vec<String>,
}

impl InsightDraft {
    fn penalize(&mut self, value: u8) {
        self.penalty = self.penalty.saturating_add(value);
    }

    fn suggest(&mut self, suggestion: impl Into<String>) {
        self.suggestions.push(suggestion.into());
    }
}

pub struct ErrorRatioRule;
pub struct SlowRatioRule;
pub struct ParseErrorRule;
pub struct ServiceSkewRule;

impl Rule for ErrorRatioRule {
    fn apply(&self, result: &AnalysisResult, draft: &mut InsightDraft) {
        let ratio = result.error_ratio();
        if ratio >= 0.30 {
            draft.penalize(45);
            draft.suggest(format!(
                "错误日志占比达到 {:.1}%，建议优先排查 ERROR/FATAL 记录。",
                ratio * 100.0
            ));
        } else if ratio >= 0.10 {
            draft.penalize(25);
            draft.suggest(format!(
                "错误日志占比为 {:.1}%，建议查看错误集中出现的服务。",
                ratio * 100.0
            ));
        } else if ratio > 0.0 {
            draft.penalize(8);
            draft.suggest(format!(
                "存在少量错误日志，占比 {:.1}%，可作为后续优化项。",
                ratio * 100.0
            ));
        }
    }
}

impl Rule for SlowRatioRule {
    fn apply(&self, result: &AnalysisResult, draft: &mut InsightDraft) {
        let ratio = result.slow_ratio();
        if ratio >= 0.40 {
            draft.penalize(35);
            draft.suggest(format!(
                "慢请求占比达到 {:.1}%，建议检查数据库、缓存和下游接口耗时。",
                ratio * 100.0
            ));
        } else if ratio >= 0.15 {
            draft.penalize(18);
            draft.suggest(format!(
                "慢请求占比为 {:.1}%，建议进一步按 service 维度定位。",
                ratio * 100.0
            ));
        } else if result.latency.slow_count > 0 {
            draft.penalize(5);
            draft.suggest("发现少量慢请求，可以结合原始日志进行抽样排查。".to_string());
        }
    }
}

impl Rule for ParseErrorRule {
    fn apply(&self, result: &AnalysisResult, draft: &mut InsightDraft) {
        if result.parse_errors.is_empty() {
            return;
        }
        let ratio = result.parse_error_ratio();
        if ratio >= 0.20 {
            draft.penalize(20);
            draft.suggest(format!(
                "解析失败行占比 {:.1}%，建议统一日志格式。",
                ratio * 100.0
            ));
        } else {
            draft.penalize(6);
            draft
                .suggest("存在个别无法解析的日志行，建议检查是否缺少 key=value 结构。".to_string());
        }
    }
}

impl Rule for ServiceSkewRule {
    fn apply(&self, result: &AnalysisResult, draft: &mut InsightDraft) {
        let Some(first) = result.top_services.first() else {
            return;
        };
        let total = result.parsed_records.max(1) as f64;
        let ratio = first.count as f64 / total;
        if ratio >= 0.70 {
            draft.penalize(10);
            draft.suggest(format!(
                "服务 `{}` 占全部记录的 {:.1}%，分析结果可能被单个服务主导。",
                first.key,
                ratio * 100.0
            ));
        }
    }
}

pub fn build_insight(result: &AnalysisResult) -> Insight {
    if result.parsed_records == 0 {
        return Insight::empty();
    }

    let mut draft = InsightDraft::default();
    let rules: Vec<Box<dyn Rule>> = vec![
        Box::new(ErrorRatioRule),
        Box::new(SlowRatioRule),
        Box::new(ParseErrorRule),
        Box::new(ServiceSkewRule),
    ];
    for rule in rules {
        rule.apply(result, &mut draft);
    }

    if draft.suggestions.is_empty() {
        draft.suggest("整体日志状态较好，未发现明显错误或慢请求集中问题。".to_string());
    }

    let score = 100u8.saturating_sub(draft.penalty);
    let health = match score {
        85..=100 => HealthLevel::Healthy,
        70..=84 => HealthLevel::Notice,
        45..=69 => HealthLevel::Risky,
        _ => HealthLevel::Critical,
    };
    Insight {
        health,
        score,
        error_ratio: result.error_ratio(),
        slow_ratio: result.slow_ratio(),
        dominant_service: result.top_services.first().map(|item| item.key.clone()),
        suggestions: draft.suggestions,
    }
}

pub trait ReportRenderer {
    fn render(&self, result: &AnalysisResult) -> String;
}

#[derive(Debug, Clone)]
pub struct TextRenderer {
    pub top_n: usize,
}

#[derive(Debug, Clone)]
pub struct MarkdownRenderer {
    pub top_n: usize,
}

#[derive(Debug, Clone)]
pub struct JsonRenderer {
    pub top_n: usize,
}

#[derive(Debug, Clone)]
pub struct CsvRenderer;

impl ReportRenderer for TextRenderer {
    fn render(&self, result: &AnalysisResult) -> String {
        let mut out = String::new();
        push_line(&mut out, "Rust Log Lab - Analysis Report");
        push_line(&mut out, "================================");
        push_line(
            &mut out,
            &format!("total lines      : {}", result.total_lines),
        );
        push_line(
            &mut out,
            &format!("parsed records   : {}", result.parsed_records),
        );
        push_line(
            &mut out,
            &format!("skipped lines    : {}", result.skipped_lines),
        );
        push_line(
            &mut out,
            &format!("parse errors     : {}", result.parse_errors.len()),
        );
        push_line(&mut out, "");

        push_line(&mut out, "Level distribution");
        for level in [
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
            LogLevel::Fatal,
            LogLevel::Unknown,
        ] {
            let count = result
                .level_stats
                .by_level
                .get(&level)
                .copied()
                .unwrap_or(0);
            if count > 0 {
                push_line(&mut out, &format!("  {:<7} {}", level, count));
            }
        }
        push_line(&mut out, "");

        push_line(&mut out, "Latency");
        push_line(&mut out, &format!("  samples : {}", result.latency.count));
        push_line(
            &mut out,
            &format!(
                "  min/max : {:?}/{:?} ms",
                result.latency.min, result.latency.max
            ),
        );
        push_line(
            &mut out,
            &format!(
                "  average : {} ms",
                result
                    .latency
                    .avg()
                    .map(|v| format!("{v:.2}"))
                    .unwrap_or_else(|| "N/A".to_string())
            ),
        );
        push_line(
            &mut out,
            &format!("  slow    : {}", result.latency.slow_count),
        );
        push_line(&mut out, "");

        let insight = build_insight(result);
        push_line(&mut out, "Health insight");
        push_line(&mut out, &format!("  health  : {}", insight.health));
        push_line(&mut out, &format!("  score   : {}", insight.score));
        push_line(
            &mut out,
            &format!("  errors  : {:.1}%", insight.error_ratio * 100.0),
        );
        push_line(
            &mut out,
            &format!("  slow    : {:.1}%", insight.slow_ratio * 100.0),
        );
        if let Some(service) = &insight.dominant_service {
            push_line(&mut out, &format!("  main svc: {}", service));
        }
        for suggestion in &insight.suggestions {
            push_line(&mut out, &format!("  - {}", suggestion));
        }
        push_line(&mut out, "");

        push_line(&mut out, &format!("Top {} services", self.top_n));
        for item in &result.top_services {
            push_line(&mut out, &format!("  {:<16} {}", item.key, item.count));
        }
        push_line(&mut out, "");

        push_line(&mut out, "Error-like records");
        for record in result.error_records.iter().take(self.top_n) {
            push_line(&mut out, &format_record_text(record));
        }
        push_line(&mut out, "");

        push_line(&mut out, "Slow records");
        for record in result.slow_records.iter().take(self.top_n) {
            push_line(&mut out, &format_record_text(record));
        }

        if !result.parse_errors.is_empty() {
            push_line(&mut out, "");
            push_line(&mut out, "Parse errors");
            for err in result.parse_errors.iter().take(self.top_n) {
                push_line(&mut out, &format!("  {err}"));
            }
        }
        out
    }
}

impl ReportRenderer for MarkdownRenderer {
    fn render(&self, result: &AnalysisResult) -> String {
        let insight = build_insight(result);
        let mut out = String::new();
        push_line(&mut out, "# Rust Log Lab 日志分析报告");
        push_line(&mut out, "");
        push_line(&mut out, "## 1. 基本信息");
        push_line(&mut out, "");
        push_line(&mut out, &format!("- 总日志行数：{}", result.total_lines));
        push_line(
            &mut out,
            &format!("- 成功解析记录：{}", result.parsed_records),
        );
        push_line(&mut out, &format!("- 跳过行数：{}", result.skipped_lines));
        push_line(
            &mut out,
            &format!("- 解析错误：{}", result.parse_errors.len()),
        );
        push_line(
            &mut out,
            &format!("- WARN 数量：{}", result.warning_count()),
        );
        push_line(
            &mut out,
            &format!("- ERROR/FATAL 数量：{}", result.error_like_count()),
        );
        push_line(&mut out, "");

        push_line(&mut out, "## 2. 日志等级分布");
        push_line(&mut out, "");
        push_line(&mut out, "| 等级 | 数量 |");
        push_line(&mut out, "|---|---:|");
        for level in [
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
            LogLevel::Fatal,
            LogLevel::Unknown,
        ] {
            let count = result
                .level_stats
                .by_level
                .get(&level)
                .copied()
                .unwrap_or(0);
            if count > 0 {
                push_line(&mut out, &format!("| {} | {} |", level, count));
            }
        }
        push_line(&mut out, "");

        push_line(&mut out, "## 3. 健康评估");
        push_line(&mut out, "");
        push_line(&mut out, &format!("- 健康状态：{}", insight.health));
        push_line(&mut out, &format!("- 健康分数：{}", insight.score));
        push_line(
            &mut out,
            &format!("- 错误占比：{:.1}%", insight.error_ratio * 100.0),
        );
        push_line(
            &mut out,
            &format!("- 慢请求占比：{:.1}%", insight.slow_ratio * 100.0),
        );
        if !result.has_warnings_or_errors() {
            push_line(&mut out, "- 当前过滤结果中没有 WARN/ERROR/FATAL 日志。");
        }
        if let Some(service) = &insight.dominant_service {
            push_line(&mut out, &format!("- 主要服务：{}", service));
        }
        for suggestion in &insight.suggestions {
            push_line(&mut out, &format!("- 建议：{}", suggestion));
        }
        push_line(&mut out, "");

        push_line(&mut out, &format!("## 4. Top {} 服务", self.top_n));
        push_line(&mut out, "");
        push_line(&mut out, "| 服务 | 数量 |");
        push_line(&mut out, "|---|---:|");
        for item in &result.top_services {
            push_line(
                &mut out,
                &format!("| {} | {} |", escape_markdown(&item.key), item.count),
            );
        }
        push_line(&mut out, "");

        push_line(&mut out, "## 5. 错误日志样例");
        push_line(&mut out, "");
        if result.error_records.is_empty() {
            push_line(&mut out, "暂无错误日志样例。");
        } else {
            for record in result.error_records.iter().take(self.top_n) {
                push_line(&mut out, &format!("- {}", format_record_markdown(record)));
            }
        }
        push_line(&mut out, "");

        push_line(&mut out, "## 6. 慢请求样例");
        push_line(&mut out, "");
        if result.slow_records.is_empty() {
            push_line(&mut out, "暂无慢请求样例。");
        } else {
            for record in result.slow_records.iter().take(self.top_n) {
                push_line(&mut out, &format!("- {}", format_record_markdown(record)));
            }
        }
        out
    }
}

impl ReportRenderer for JsonRenderer {
    fn render(&self, result: &AnalysisResult) -> String {
        let mut out = String::new();
        push_line(&mut out, "{");
        push_line(
            &mut out,
            &format!("  \"total_lines\": {},", result.total_lines),
        );
        push_line(
            &mut out,
            &format!("  \"parsed_records\": {},", result.parsed_records),
        );
        push_line(
            &mut out,
            &format!("  \"skipped_lines\": {},", result.skipped_lines),
        );
        push_line(&mut out, "  \"levels\": {");
        let mut first = true;
        for (level, count) in &result.level_stats.by_level {
            if !first {
                push_line(&mut out, ",");
            }
            out.push_str(&format!("    \"{}\": {}", level.as_str(), count));
            first = false;
        }
        push_line(&mut out, "");
        push_line(&mut out, "  },");
        push_line(&mut out, "  \"latency\": {");
        push_line(
            &mut out,
            &format!("    \"count\": {},", result.latency.count),
        );
        push_line(
            &mut out,
            &format!("    \"min\": {},", option_u64(result.latency.min)),
        );
        push_line(
            &mut out,
            &format!("    \"max\": {},", option_u64(result.latency.max)),
        );
        push_line(
            &mut out,
            &format!(
                "    \"average\": {},",
                result
                    .latency
                    .avg()
                    .map(|v| format!("{v:.2}"))
                    .unwrap_or_else(|| "null".to_string())
            ),
        );
        push_line(
            &mut out,
            &format!("    \"slow_count\": {}", result.latency.slow_count),
        );
        push_line(&mut out, "  },");
        let insight = build_insight(result);
        push_line(&mut out, "  \"insight\": {");
        push_line(
            &mut out,
            &format!("    \"health\": \"{}\",", insight.health),
        );
        push_line(&mut out, &format!("    \"score\": {},", insight.score));
        push_line(
            &mut out,
            &format!("    \"error_ratio\": {:.4},", insight.error_ratio),
        );
        push_line(
            &mut out,
            &format!("    \"slow_ratio\": {:.4}", insight.slow_ratio),
        );
        push_line(&mut out, "  },");
        push_line(&mut out, "  \"top_services\": [");
        for (idx, item) in result.top_services.iter().take(self.top_n).enumerate() {
            let comma = if idx + 1 == result.top_services.len().min(self.top_n) {
                ""
            } else {
                ","
            };
            push_line(
                &mut out,
                &format!(
                    "    {{\"service\": \"{}\", \"count\": {}}}{comma}",
                    escape_json(&item.key),
                    item.count
                ),
            );
        }
        push_line(&mut out, "  ],");
        push_line(&mut out, "  \"parse_errors\": [");
        for (idx, err) in result.parse_errors.iter().take(self.top_n).enumerate() {
            let comma = if idx + 1 == result.parse_errors.len().min(self.top_n) {
                ""
            } else {
                ","
            };
            push_line(&mut out, &format!("    \"{}\"{comma}", escape_json(err)));
        }
        push_line(&mut out, "  ]");
        push_line(&mut out, "}");
        out
    }
}

impl ReportRenderer for CsvRenderer {
    fn render(&self, result: &AnalysisResult) -> String {
        let mut out = String::new();
        push_line(&mut out, "metric,key,value");
        push_line(
            &mut out,
            &format!("summary,total_lines,{}", result.total_lines),
        );
        push_line(
            &mut out,
            &format!("summary,parsed_records,{}", result.parsed_records),
        );
        push_line(
            &mut out,
            &format!("summary,skipped_lines,{}", result.skipped_lines),
        );
        for (level, count) in &result.level_stats.by_level {
            push_line(&mut out, &format!("level,{},{}", level, count));
        }
        for (service, count) in &result.service_count {
            push_line(
                &mut out,
                &format!("service,{},{}", escape_csv(service), count),
            );
        }
        push_line(&mut out, &format!("latency,count,{}", result.latency.count));
        if let Some(avg) = result.latency.avg() {
            push_line(&mut out, &format!("latency,average,{avg:.2}"));
        }
        push_line(
            &mut out,
            &format!("latency,slow_count,{}", result.latency.slow_count),
        );
        out
    }
}

pub fn render_report(result: &AnalysisResult, format: OutputFormat, top_n: usize) -> String {
    match format {
        OutputFormat::Text => TextRenderer { top_n }.render(result),
        OutputFormat::Markdown => MarkdownRenderer { top_n }.render(result),
        OutputFormat::Json => JsonRenderer { top_n }.render(result),
        OutputFormat::Csv => CsvRenderer.render(result),
    }
}

fn format_record_text(record: &crate::model::LogRecord) -> String {
    let request = record
        .field("request_id")
        .or_else(|| record.field("trace_id"))
        .map(|value| format!(" request={value}"))
        .unwrap_or_default();
    let latency = record
        .latency_ms
        .map(|value| format!(" {value}ms"))
        .unwrap_or_default();
    format!(
        "  line {:<4} {:<8} {:<7}{}{} {}",
        record.line_number, record.service, record.level, latency, request, record.message
    )
}

fn format_record_markdown(record: &crate::model::LogRecord) -> String {
    let request = record
        .field("request_id")
        .or_else(|| record.field("trace_id"))
        .map(|value| format!(", request `{}`", escape_markdown(value)))
        .unwrap_or_default();
    let latency = record
        .latency_ms
        .map(|value| format!(", 耗时 `{value}ms`"))
        .unwrap_or_default();
    format!(
        "第 {} 行，服务 `{}`，等级 `{}`{}{}，消息：{}",
        record.line_number,
        escape_markdown(&record.service),
        record.level,
        latency,
        request,
        escape_markdown(&record.message)
    )
}

fn push_line(out: &mut String, line: &str) {
    out.push_str(line);
    out.push('\n');
}

fn option_u64(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn escape_json(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn escape_csv(input: &str) -> String {
    if input.contains(',') || input.contains('"') || input.contains('\n') {
        format!("\"{}\"", input.replace('"', "\"\""))
    } else {
        input.to_string()
    }
}

fn escape_markdown(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "\\`")
        .replace('*', "\\*")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::{analyze_lines, AnalyzeOptions};

    #[test]
    fn markdown_report_contains_sections() {
        let lines =
            vec!["service=auth level=ERROR latency=10 request_id=req-1 msg=bad".to_string()];
        let result = analyze_lines(&lines, AnalyzeOptions::default()).unwrap();
        let report = render_report(&result, OutputFormat::Markdown, 5);
        assert!(report.contains("# Rust Log Lab 日志分析报告"));
        assert!(report.contains("request `req-1`"));
    }

    #[test]
    fn text_report_contains_title() {
        let lines = vec!["service=auth level=INFO latency=1 msg=ok".to_string()];
        let result = analyze_lines(&lines, AnalyzeOptions::default()).unwrap();
        let report = render_report(&result, OutputFormat::Text, 5);
        assert!(report.contains("Rust Log Lab"));
    }

    #[test]
    fn insight_is_healthy_when_no_error_or_slow_records() {
        let lines = vec![
            "service=auth level=INFO latency=10 msg=ok".to_string(),
            "service=pay level=INFO latency=20 msg=ok".to_string(),
        ];
        let result = analyze_lines(&lines, AnalyzeOptions::default()).unwrap();
        let insight = build_insight(&result);
        assert_eq!(insight.health, HealthLevel::Healthy);
    }

    #[test]
    fn insight_becomes_risky_when_many_errors_exist() {
        let lines = vec![
            "service=auth level=ERROR latency=10 msg=bad".to_string(),
            "service=pay level=ERROR latency=20 msg=bad".to_string(),
            "service=pay level=INFO latency=20 msg=ok".to_string(),
        ];
        let result = analyze_lines(&lines, AnalyzeOptions::default()).unwrap();
        let insight = build_insight(&result);
        assert!(matches!(
            insight.health,
            HealthLevel::Risky | HealthLevel::Critical
        ));
    }

    #[test]
    fn json_report_contains_insight_block() {
        let lines = vec!["service=auth level=ERROR latency=900 msg=bad".to_string()];
        let result = analyze_lines(&lines, AnalyzeOptions::default()).unwrap();
        let report = render_report(&result, OutputFormat::Json, 5);
        assert!(report.contains("\"insight\""));
        assert!(report.contains("\"score\""));
    }
}
