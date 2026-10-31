# Research: 日志 TRACE 分级与轮转命名优化

**Feature**: `006-log-trace-level` | **Date**: 2026-10-31

## R1. CLI 级别开关与优先级

**Decision**: `Args` 增加布尔字段 `trace`。生效顺序为：`--trace` → trace；否则 `--debug` → debug；否则沿用配置文件级别（默认 info）。两个开关同时出现时不报错，TRACE 优先。

**Rationale**: 规格 FR-002 与 Edge Cases 明确要求 `--trace` 优先于 `--debug`；用简单 `if/else if` 覆盖即可保持 clap 参数表最小。将覆盖逻辑提取为独立函数，便于单元测试四种组合。

**Alternatives considered**:
- clap `conflicts_with = "debug"`：同时指定时直接报错，与规格“trace 优先”冲突。
- 单一 `--log-level <level>` 参数：更通用但改变现有 CLI 习惯，超出本次需求。

## R2. DEBUG 与 TRACE 的职责划分

**Decision**: TRACE 输出原始报文字节（十六进制列表）；DEBUG 输出解析后的可读摘要。TRACE 为包含 DEBUG 的更高详细度，因此 TRACE 模式下两类记录同时出现。

**Rationale**: 已在 clarify 阶段确认。原始字节噪声大，只在排障时开启；日常 DEBUG 只需看懂类型、事务 ID 与属性。`log` 级别过滤天然实现包含关系，无需额外分支。

**Alternatives considered**:
- DEBUG 原始字节、TRACE 内部解码阶段：日志量最大且信息重复，被用户否决。
- 两级都输出原始字节：无增量价值。

## R3. 消息摘要的格式与实现位置

**Decision**: 在 `src/server/server.rs` 增加私有函数：
- `describe_message(&StunMessage) -> String`
- `describe_attribute(&StunAttribute, magic_cookie, transaction_id) -> String`

输出格式：
```text
Received: Binding Request txn=09e96d1c0e8ae31800000000 attrs=[SOFTWARE="test-client"]
Response: Binding Success Response txn=09e96d1c0e8ae31800000000 attrs=[XOR-MAPPED-ADDRESS=127.0.0.1:53466]
```
规则：
- 消息类型名称：`Binding Request`、`Binding Success Response`、`Binding Error Response`
- 事务 ID：12 字节 → 24 位小写十六进制
- 属性：已知类型显示名称；`MAPPED-ADDRESS`/`SOURCE-ADDRESS`/`CHANGED-ADDRESS`/`OTHER-ADDRESS` 解码为 `ip:port`；`XOR-MAPPED-ADDRESS` 用消息的 magic cookie 与事务 ID 解码；`SOFTWARE` 按 UTF-8（lossy）显示；解码失败回退为 `NAME(len=n)`
- 未知属性：`0xXXXX(len=n)`，使用原始类型码
- 空属性列表：`attrs=[]`

**Rationale**: 摘要格式属于服务端可观测性关注点，放在服务端避免协议库承担展示职责；纯函数便于 TDD。单行摘要便于 grep 与日志采集。

**Alternatives considered**:
- 实现 `Display for StunMessage`：会锁定协议库的展示格式，影响其他使用方。
- 多行/JSON 摘要：可解析性更好，但与现有单行日志风格不一致，且需求未要求。

## R4. 未知属性类型码的保留

**Decision**: 为 `StunAttribute` 增加 `pub raw_type: u16` 字段；`new()` 用 `attribute_type as u16` 初始化，`decode()` 保存线上原始值，`encode()` 写回 `raw_type`。

**Rationale**: 当前 `StunAttributeType::from_raw` 将未知类型统一折叠为 `0xFFFF`，摘要无法展示真实类型码，且往返编码会破坏未知属性。保留原始值同时满足 FR-006 的可读摘要与 RFC 5389 对未知属性的透传要求。

**Alternatives considered**:
- 摘要中统一显示 `UNKNOWN`：不满足“未知属性以类型码和长度呈现”的验收场景。
- 将 `StunAttributeType` 改为携带 `Other(u16)`：改动面大，影响现有模式匹配。

## R5. 日志轮转命名与跨天语义

**Decision**:
- 轮转文件名为 `{active_path}.{YYYY-MM-DD}.{N}`，例如 `stunserver.log.2026-10-31.1`。
- `N` 从 1 开始、不补零；每次轮转从 1 起扫描同日期文件名，找到第一个不存在的 N，天然实现递增与不覆盖。
- 大小轮转使用当天日期；跨天轮转使用 `current_date`（被归档内容所属的日期）命名，随后把 `current_date` 更新为今天。
- `prune_old_files` 逻辑不变，仍按 `{file_stem}.*` 匹配并按修改时间删除最旧文件；历史无序号文件继续参与保留策略。

**Rationale**: 文件名直接表达“内容所属日期 + 当天序号”，满足可归档性；无需持久化计数器，重启后仍能正确续号；不覆盖保证历史可追溯。跨天时用旧日期命名可避免把前一天内容错误标记为今天。

**Alternatives considered**:
- 持久化全局序号：增加状态与故障恢复复杂度，且跨天需额外重置逻辑。
- 固定宽度补零：用户在 clarify 中明确选择不补零。
- 跨天使用新日期命名旧内容：会导致日志所属日期失真，被否决。

## R6. 请求处理路径与解码复用

**Decision**: 把“由已解码请求构造响应消息”提取为内部函数（例如 `build_response_message(&StunMessage, SocketAddr) -> StunMessage`）。接收循环中：
1. 收到数据后立即 `trace!("Received: {:02x?}", data)`
2. 解码一次；合法请求输出 DEBUG 接收摘要
3. 构造响应后输出 DEBUG 响应摘要，再 `trace!("Response: {:02x?}", response_bytes)`
4. 无效请求输出 `debug!("Invalid STUN request from {}", addr)` 并计入错误

公开的 `build_response(&[u8], SocketAddr) -> Option<Vec<u8>>` 保留为薄包装，现有单测与集成测试无需改动。

**Rationale**: 避免为摘要重复解码；保持既有公共 API 稳定；响应摘要与发送字节来自同一对象，避免不一致。

**Alternatives considered**:
- 在 `build_response` 内部打印日志：无法区分无效请求的原始字节记录，且把 I/O 关注点混入纯函数。
- 处理循环中重复解码：增加热路径 CPU 开销。

## R7. 级别门控与性能

**Decision**: 使用 `log::trace!` / `log::debug!` 宏。摘要字符串作为宏参数传入，仅在对应级别启用时才构造；原始字节格式化同理。锁与文件写入沿用现有 `StunLogger`/`FileLogger` 实现。

**Rationale**: `log` 宏在级别未启用时不会求值参数，INFO 默认配置下无额外开销；TRACE 作为显式排障选项，其格式化与 I/O 成本由用户主动承担。

**Alternatives considered**:
- 预先构造摘要字符串再判断级别：会在 INFO 下产生无谓分配。
- 采样或限流：需求未要求，且会削弱排障完整性。

## R8. 文档更新

**Decision**: 更新 `README.md` 与 `README.zh-CN.md` 的“日志轮转”章节，将示例文件名改为 `stunserver.log.YYYY-MM-DD.N`；`--trace` 通过 clap 自动生成 `--help` 说明，无需额外 CLI 文档章节。

**Rationale**: 现有 README 的 `stunserver.log.1` 示例与实现不符，且本次命名规则变更直接影响运维归档习惯。

**Alternatives considered**:
- 仅更新 CLI help：README 仍会误导用户对轮转文件名的预期。
