# 基于Rust的多线程日志分析CLI工具

本项目是 Rust 程序设计课程期末大作业，实现了一个离线命令行日志分析工具，可以读取 `key=value` 格式的服务日志，统计日志级别分布、服务访问量、慢请求、错误记录，并支持 text/markdown/json/csv 四种报告输出。迭代版本新增了报告导出模块，可以将分析结果保存为本地报告文件。

## 一、项目特点

- 使用 Rust 2021 Edition 编写，主要逻辑不依赖第三方库。
- 使用 `mod` 进行模块化组织：`cli`、`parser`、`model`、`analyzer`、`report`、`export`、`error`。其中 `export` 是迭代新增的报告导出模块，负责将分析结果写入本地文件或终端。
- 使用 `Result<T, AppError>` 统一错误处理，避免在业务流程中大量使用 `unwrap/expect`。
- 使用 `struct`、`enum`、`trait`、泛型集合、所有权转移与借用。
- 使用 `std::thread` 和 `std::sync::mpsc` 实现多线程分析。
- 包含单元测试与关键功能测试。

## 二、项目结构

```text
rust_log_lab/
├── Cargo.toml
├── README.md
├── examples/
│   └── sample.log
└── src/
    ├── analyzer.rs  # 多线程调度、统计器、结果合并
    ├── cli.rs       # 命令行参数解析、文件读取与 demo 数据
    ├── error.rs     # 自定义错误类型
    ├── export.rs    # 报告导出模块，支持写入文件和格式推断
    ├── lib.rs       # 公共模块导出与集成测试
    ├── main.rs      # 程序入口
    ├── model.rs     # 核心数据结构和枚举
    ├── parser.rs    # key=value 日志解析器
    └── report.rs    # text/markdown/json/csv 报告渲染与健康评估
```

## 三、编译运行

### 1. 查看帮助

```bash
cargo run -- help
```

### 2. 运行内置 demo

```bash
cargo run -- demo
```

### 3. 分析示例日志

```bash
cargo run -- analyze examples/sample.log
```

### 4. 按级别过滤

```bash
cargo run -- analyze examples/sample.log --level WARN
```

### 5. 只分析某个服务

```bash
cargo run -- analyze examples/sample.log --service auth
```

### 6. 输出 Markdown 报告

```bash
cargo run -- analyze examples/sample.log --out report.md
```

也可以显式指定格式：

```bash
cargo run -- analyze examples/sample.log --format markdown --out report.md
```

### 7. 输出 JSON 报告

```bash
cargo run -- analyze examples/sample.log --format json --out report.json
```

### 8. 输出 CSV 报告

```bash
cargo run -- analyze examples/sample.log --format csv --out report.csv
```

## 四、日志格式

本项目支持空格分隔的 `key=value` 日志。值使用双引号包裹。

```text
service=auth level=ERROR latency=120 msg="login failed"
```

常用字段：

- `service`：服务名，例如 `auth`、`order`、`payment`。
- `level`：日志级别，例如 `DEBUG`、`INFO`、`WARN`、`ERROR`、`FATAL`。
- `latency` / `latency_ms` / `cost`：耗时，支持 `120` 或 `120ms`。
- `msg` / `message`：日志消息。

## 五、测试与规范检查

```bash
cargo test
cargo fmt
cargo clippy
```

## 六、迭代版本说明

本版本在基础日志分析功能之外，新增 `export.rs` 报告导出模块。基础版本主要将分析结果输出到终端，迭代版本可以通过 `--out` 参数将报告保存为本地文件，并能够根据文件扩展名自动推断输出格式。例如，`report.md` 会自动生成 Markdown 报告，`report.json` 会自动生成 JSON 报告，`report.csv` 会自动生成 CSV 报告。