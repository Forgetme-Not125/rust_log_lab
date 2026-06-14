# Rust Log Lab 日志分析报告

## 1. 基本信息

- 总日志行数：10
- 成功解析记录：10
- 跳过行数：0
- 解析错误：0
- WARN 数量：2
- ERROR/FATAL 数量：4

## 2. 日志等级分布

| 等级 | 数量 |
|---|---:|
| DEBUG | 1 |
| INFO | 3 |
| WARN | 2 |
| ERROR | 4 |

## 3. 健康评估

- 健康状态：CRITICAL
- 健康分数：37
- 错误占比：40.0%
- 慢请求占比：30.0%
- 主要服务：auth
- 建议：错误日志占比达到 40.0%，建议优先排查 ERROR/FATAL 记录。
- 建议：慢请求占比为 30.0%，建议进一步按 service 维度定位。

## 4. Top 5 服务

| 服务 | 数量 |
|---|---:|
| auth | 3 |
| order | 3 |
| payment | 2 |
| search | 2 |

## 5. 错误日志样例

- 第 2 行，服务 `auth`，等级 `ERROR`, 耗时 `145ms`，消息：login failed
- 第 4 行，服务 `payment`，等级 `ERROR`, 耗时 `830ms`，消息：payment timeout
- 第 5 行，服务 `auth`，等级 `ERROR`, 耗时 `120ms`，消息：password wrong
- 第 10 行，服务 `order`，等级 `ERROR`, 耗时 `1002ms`，消息：order rollback

## 6. 慢请求样例

- 第 3 行，服务 `order`，等级 `WARN`, 耗时 `610ms`，消息：slow query
- 第 4 行，服务 `payment`，等级 `ERROR`, 耗时 `830ms`，消息：payment timeout
- 第 10 行，服务 `order`，等级 `ERROR`, 耗时 `1002ms`，消息：order rollback
