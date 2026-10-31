# Quickstart Validation: 日志 TRACE 分级与轮转命名优化

**Feature**: `006-log-trace-level`

本指南用于端到端验证规格中的成功标准（SC-001 ~ SC-005）。

## 前置条件

- 已完成实现并通过构建
- 本机可运行 `stunserver` 与 `stunclient`
- 选择一个未被占用的端口（以下示例使用 `13478`）

```bash
cd /opt/stunserver
cargo build --release
```

## 场景 1：`--trace` 输出原始报文（SC-001）

```bash
./target/release/stunserver --trace --listen 127.0.0.1:13478 > /tmp/stun-trace.log 2>&1 &
SERVER_PID=$!
sleep 1
./target/release/stunclient --quiet 127.0.0.1:13478
kill -INT "$SERVER_PID"
cat /tmp/stun-trace.log
```

期望：
- 出现带毫秒时间戳的 `[INFO]` 启动日志
- 出现 `[DEBUG] Received: Binding Request txn=... attrs=[...]`
- 出现 `[DEBUG] Response: Binding Success Response txn=... attrs=[XOR-MAPPED-ADDRESS=127.0.0.1:...]`
- 出现 `[TRACE] Received: [00, 01, ...]` 与 `[TRACE] Response: [01, 01, ...]`

## 场景 2：`--debug` 不输出原始字节（SC-002）

```bash
./target/release/stunserver --debug --listen 127.0.0.1:13478 > /tmp/stun-debug.log 2>&1 &
SERVER_PID=$!
sleep 1
./target/release/stunclient --quiet 127.0.0.1:13478
kill -INT "$SERVER_PID"
cat /tmp/stun-debug.log
```

期望：
- 每个成功请求有 `Request from`、`Received:` 摘要、`Response:` 摘要各一条
- 不出现 `Received: [` / `Response: [` 形式的十六进制字节列表
- 可用以下命令确认原始字节出现次数为 0：

```bash
grep -cE '\[(DEBUG|INFO)\] (Received|Response): \[' /tmp/stun-debug.log
```

## 场景 3：摘要可读性（SC-005）

从 `--debug` 日志中直接确认：
- `Received:` 行包含消息类型、`txn=` 与 `attrs=`
- `Response:` 行包含 `XOR-MAPPED-ADDRESS=<ip>:<port>`
- 发送带 `SOFTWARE` 属性的请求时，摘要显示 `SOFTWARE="..."`（可用现有测试或客户端构造）

## 场景 4：轮转文件命名（SC-003）

轮转由大小/跨天触发，使用自动化测试验证：

```bash
cargo test -p stunserver logging
```

期望测试覆盖并断言：
- 当天首个轮转文件为 `{prefix}.log.{date}.1`
- 同名已存在时递增为 `.2`，且原文件未被覆盖
- 跨天轮转使用旧日期归档，次日序号从 1 开始
- 历史无序号轮转文件仍参与清理

手工抽查（可选）：使用小容量配置运行并产生足够日志后检查目录：

```bash
ls -1 /tmp/stun-rot/log
# 期望形如：
# stunserver.log
# stunserver.log.2026-10-31.1
# stunserver.log.2026-10-31.2
```

## 场景 5：默认 INFO 行为不变（SC-004）

```bash
./target/release/stunserver --listen 127.0.0.1:13478 > /tmp/stun-info.log 2>&1 &
SERVER_PID=$!
sleep 1
./target/release/stunclient --quiet 127.0.0.1:13478
kill -INT "$SERVER_PID"
cat /tmp/stun-info.log
```

期望：
- 仅 INFO 及更高级别日志（启动、监听、停止统计等）
- 无 `Received:` / `Response:` 记录
- 启动信息中的版本、提交、编译时间格式不变

## 配置文件级别验证（FR-013）

```toml
[logging]
level = "trace"
enable_console = true
```

使用该配置启动（不传 `--trace`），应得到与场景 1 相同的详细度。

## 质量门（提交前）

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
```
