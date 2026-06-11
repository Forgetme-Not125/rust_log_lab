use std::collections::{BTreeMap, BinaryHeap};
use std::sync::mpsc;
use std::thread;

use crate::error::{AppError, AppResult};
use crate::model::{LogFilter, LogLevel, LogRecord, TopItem};
use crate::parser::{KeyValueParser, RecordParser};

#[derive(Debug, Clone)]
pub struct AnalyzeOptions {
    pub threads: usize,
    pub top_n: usize,
    pub slow_threshold_ms: u64,
    pub filter: LogFilter,
}

impl Default for AnalyzeOptions {
    fn default() -> Self {
        Self {
            threads: 4,
            top_n: 5,
            slow_threshold_ms: 500,
            filter: LogFilter {
                min_level: None,
                service: None,
                keyword: None,
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LevelStats {
    pub total: usize,
    pub by_level: BTreeMap<LogLevel, usize>,
}

#[derive(Debug, Clone, Default)]
pub struct LatencyStats {
    pub count: usize,
    pub sum: u64,
    pub min: Option<u64>,
    pub max: Option<u64>,
    pub slow_count: usize,
}

impl LatencyStats {
    pub fn add(&mut self, value: u64, slow_threshold_ms: u64) {
        self.count += 1;
        self.sum += value;
        self.min = Some(self.min.map_or(value, |old| old.min(value)));
        self.max = Some(self.max.map_or(value, |old| old.max(value)));
        if value >= slow_threshold_ms {
            self.slow_count += 1;
        }
    }

    pub fn avg(&self) -> Option<f64> {
        if self.count == 0 {
            None
        } else {
            Some(self.sum as f64 / self.count as f64)
        }
    }

    pub fn merge(&mut self, other: &LatencyStats) {
        self.count += other.count;
        self.sum += other.sum;
        self.slow_count += other.slow_count;
        self.min = match (self.min, other.min) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        self.max = match (self.max, other.max) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
    }
}

#[derive(Debug, Clone, Default)]
pub struct AnalysisResult {
    pub total_lines: usize,
    pub parsed_records: usize,
    pub skipped_lines: usize,
    pub parse_errors: Vec<String>,
    pub level_stats: LevelStats,
    pub service_count: BTreeMap<String, usize>,
    pub latency: LatencyStats,
    pub top_services: Vec<TopItem<String>>,
    pub slow_records: Vec<LogRecord>,
    pub error_records: Vec<LogRecord>,
}

#[allow(dead_code)]
impl AnalysisResult {
    pub fn error_like_count(&self) -> usize {
        self.level_stats
            .by_level
            .get(&LogLevel::Error)
            .copied()
            .unwrap_or(0)
            + self
                .level_stats
                .by_level
                .get(&LogLevel::Fatal)
                .copied()
                .unwrap_or(0)
    }

    pub fn warning_count(&self) -> usize {
        self.level_stats
            .by_level
            .get(&LogLevel::Warn)
            .copied()
            .unwrap_or(0)
    }

    pub fn error_ratio(&self) -> f64 {
        if self.parsed_records == 0 {
            0.0
        } else {
            self.error_like_count() as f64 / self.parsed_records as f64
        }
    }

    pub fn slow_ratio(&self) -> f64 {
        if self.latency.count == 0 {
            0.0
        } else {
            self.latency.slow_count as f64 / self.latency.count as f64
        }
    }

    pub fn parse_error_ratio(&self) -> f64 {
        if self.total_lines == 0 {
            0.0
        } else {
            self.parse_errors.len() as f64 / self.total_lines as f64
        }
    }

    pub fn has_warnings_or_errors(&self) -> bool {
        self.error_like_count() > 0 || self.warning_count() > 0
    }

    pub fn merge(&mut self, other: AnalysisResult, top_n: usize) {
        self.total_lines += other.total_lines;
        self.parsed_records += other.parsed_records;
        self.skipped_lines += other.skipped_lines;
        self.parse_errors.extend(other.parse_errors);

        self.level_stats.total += other.level_stats.total;
        for (level, count) in other.level_stats.by_level {
            *self.level_stats.by_level.entry(level).or_insert(0) += count;
        }
        for (service, count) in other.service_count {
            *self.service_count.entry(service).or_insert(0) += count;
        }
        self.latency.merge(&other.latency);
        self.slow_records.extend(other.slow_records);
        self.error_records.extend(other.error_records);
        self.slow_records.sort_by_key(|record| record.line_number);
        self.error_records.sort_by_key(|record| record.line_number);
        self.slow_records.truncate(top_n.max(1) * 10);
        self.error_records.truncate(top_n.max(1) * 10);
        self.top_services = top_items(&self.service_count, top_n);
    }
}

pub trait Collector {
    fn collect(&mut self, record: &LogRecord);
    fn finish(self) -> AnalysisResult;
}

pub struct StatsCollector {
    options: AnalyzeOptions,
    result: AnalysisResult,
}

impl StatsCollector {
    pub fn new(options: AnalyzeOptions) -> Self {
        Self {
            options,
            result: AnalysisResult::default(),
        }
    }

    pub fn skip_line(&mut self) {
        self.result.skipped_lines += 1;
    }

    pub fn add_parse_error(&mut self, line_number: usize, error: AppError) {
        self.result
            .parse_errors
            .push(format!("line {line_number}: {error}"));
    }
}

impl Collector for StatsCollector {
    fn collect(&mut self, record: &LogRecord) {
        if !self.options.filter.accept(record) {
            return;
        }
        self.result.parsed_records += 1;
        self.result.level_stats.total += 1;
        *self
            .result
            .level_stats
            .by_level
            .entry(record.level)
            .or_insert(0) += 1;
        *self
            .result
            .service_count
            .entry(record.service.clone())
            .or_insert(0) += 1;
        if let Some(latency) = record.latency_ms {
            self.result
                .latency
                .add(latency, self.options.slow_threshold_ms);
            if latency >= self.options.slow_threshold_ms {
                self.result.slow_records.push(record.clone());
            }
        }
        if record.is_error_like() {
            self.result.error_records.push(record.clone());
        }
    }

    fn finish(mut self) -> AnalysisResult {
        self.result.top_services = top_items(&self.result.service_count, self.options.top_n);
        self.result
    }
}

pub fn analyze_lines(lines: &[String], options: AnalyzeOptions) -> AppResult<AnalysisResult> {
    if lines.is_empty() {
        return Err(AppError::EmptyInput);
    }
    let thread_count = options.threads.max(1).min(lines.len());
    let chunk_size = lines.len().div_ceil(thread_count);
    let (sender, receiver) = mpsc::channel::<AnalysisResult>();
    let mut handles = Vec::new();

    for (chunk_index, chunk) in lines.chunks(chunk_size).enumerate() {
        let worker_lines: Vec<(usize, String)> = chunk
            .iter()
            .enumerate()
            .map(|(offset, line)| (chunk_index * chunk_size + offset + 1, line.clone()))
            .collect();
        let sender = sender.clone();
        let worker_options = options.clone();
        let handle = thread::spawn(move || {
            let parser = KeyValueParser::new();
            let mut collector = StatsCollector::new(worker_options);
            for (line_number, line) in worker_lines {
                collector.result.total_lines += 1;
                match parser.parse_line(line_number, &line) {
                    Ok(Some(record)) => collector.collect(&record),
                    Ok(None) => collector.skip_line(),
                    Err(err) => collector.add_parse_error(line_number, err),
                }
            }
            let _ = sender.send(collector.finish());
        });
        handles.push(handle);
    }
    drop(sender);

    let mut result = AnalysisResult::default();
    for partial in receiver {
        result.merge(partial, options.top_n);
    }

    for handle in handles {
        handle
            .join()
            .map_err(|_| AppError::Thread("worker thread panicked".to_string()))?;
    }
    result.top_services = top_items(&result.service_count, options.top_n);
    Ok(result)
}

pub fn top_items(map: &BTreeMap<String, usize>, limit: usize) -> Vec<TopItem<String>> {
    let mut heap = BinaryHeap::new();
    for (key, count) in map {
        heap.push(TopItem {
            key: key.clone(),
            count: *count,
        });
    }
    let mut out = Vec::new();
    for _ in 0..limit {
        if let Some(item) = heap.pop() {
            out.push(item);
        } else {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_counts_levels() {
        let lines = vec![
            "service=auth level=ERROR latency=100 msg=bad".to_string(),
            "service=auth level=INFO latency=10 msg=ok".to_string(),
            "service=pay level=WARN latency=600 msg=slow".to_string(),
        ];
        let options = AnalyzeOptions {
            threads: 2,
            top_n: 2,
            slow_threshold_ms: 500,
            filter: LogFilter {
                min_level: None,
                service: None,
                keyword: None,
            },
        };
        let result = analyze_lines(&lines, options).unwrap();
        assert_eq!(result.parsed_records, 3);
        assert_eq!(result.level_stats.by_level.get(&LogLevel::Error), Some(&1));
        assert_eq!(result.latency.slow_count, 1);
    }

    #[test]
    fn analysis_result_helper_methods_work() {
        let lines = vec![
            "service=auth level=ERROR latency=100 msg=bad".to_string(),
            "service=auth level=WARN latency=700 msg=slow".to_string(),
            "service=auth level=INFO latency=20 msg=ok".to_string(),
        ];
        let result = analyze_lines(&lines, AnalyzeOptions::default()).unwrap();
        assert_eq!(result.error_like_count(), 1);
        assert_eq!(result.warning_count(), 1);
        assert!(result.has_warnings_or_errors());
        assert!(result.error_ratio() > 0.0);
        assert!(result.slow_ratio() > 0.0);
    }
}
