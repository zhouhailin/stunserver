# Data Model: 日志 TRACE 分级与轮转命名优化

**Feature**: `006-log-trace-level` | **Date**: 2026-10-31

本特性不引入持久化业务数据，模型集中于日志记录、消息摘要与轮转文件命名。

## 1. 有效日志级别（EffectiveLogLevel）

| 字段 | 类型 | 说明 |
|------|------|------|
| `configured_level` | 枚举字符串 | 配置文件 `logging.level`，默认 `info`；合法值 `trace/debug/info/warn/warning/error` |
| `cli_debug` | 布尔 | 是否指定 `--debug` |
| `cli_trace` | 布尔 | 是否指定 `--trace` |
| `effective_level` | 枚举 | 最终级别：`cli_trace → trace`；否则 `cli_debug → debug`；否则 `configured_level` |

**验证规则**
- 级别名称大小写不敏感（沿用 `level_filter()`）。
- 未知配置值在加载阶段报 `InvalidLogLevel`，不进入运行期。
- `--trace` 与 `--debug` 同时存在时不报错，`effective_level = trace`。

**状态转换**

```text
未指定开关 ──> configured_level
--debug ─────> debug
--trace ─────> trace
--debug + --trace ──> trace
```

## 2. 日志记录（LogRecord）

| 字段 | 类型 | 说明 |
|------|------|------|
| `timestamp` | 本地时间，毫秒精度 | 格式 `YYYY-MM-DDTHH:MM:SS.sss` |
| `level` | 枚举 | TRACE / DEBUG / INFO / WARN / ERROR |
| `message` | 字符串（单行） | 日志正文 |

**验证规则**
- 控制台与文件输出使用同一时间戳格式。
- 新日志沿用现有毫秒精度（FR-009）。

## 3. 消息摘要（MessageSummary）

| 字段 | 类型 | 说明 |
|------|------|------|
| `message_kind` | 枚举 | `Binding Request` / `Binding Success Response` / `Binding Error Response` |
| `transaction_id` | 24 位小写十六进制字符串 | 来自 12 字节事务 ID |
| `attributes` | `AttributeSummary` 列表 | 按报文中的顺序排列；可为空 |

**验证规则**
- 仅在 DEBUG 或 TRACE 级别生成与输出。
- 请求摘要由解码成功的请求生成；响应摘要由实际发送的响应消息生成。
- 同一请求的摘要事务 ID 与响应摘要一致。

**展示形式**

```text
Received: <message_kind> txn=<transaction_id> attrs=[<attr>, ...]
Response: <message_kind> txn=<transaction_id> attrs=[<attr>, ...]
```

## 4. 属性摘要（AttributeSummary）

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | 字符串 | 已知类型显示名称；未知类型显示 `0xXXXX` |
| `raw_type` | u16 | 线上原始类型码（未知属性亦真实保留） |
| `length` | u16 | 属性值长度（字节） |
| `decoded_value` | 可选字符串 | 地址类属性的 `ip:port`、SOFTWARE 的文本等；解码失败时为空 |

**展示规则**

| 属性类型 | 展示 |
|----------|------|
| MAPPED-ADDRESS / SOURCE-ADDRESS / CHANGED-ADDRESS / OTHER-ADDRESS | `NAME=ip:port` |
| XOR-MAPPED-ADDRESS | 使用消息 magic cookie + 事务 ID 解码后显示 `XOR-MAPPED-ADDRESS=ip:port` |
| SOFTWARE | `SOFTWARE="<utf-8 lossy>"` |
| 其他已知类型 | `NAME(len=n)` |
| 未知类型 | `0xXXXX(len=n)`，XXXX 为原始类型码大写十六进制 |
| 地址解码失败 | `NAME(len=n)`，不得输出部分/错误地址 |

## 5. 原始报文记录（RawPacketRecord）

| 字段 | 类型 | 说明 |
|------|------|------|
| `direction` | 枚举 | `Received` / `Response` |
| `bytes` | 字节序列 | 完整 UDP 载荷（接收）或完整响应字节（发送前） |

**验证规则**
- 仅在 TRACE 级别输出。
- 格式沿用 `[00, 01, ...]` 小写十六进制列表。
- 接收报文在解码前记录，因此无效报文同样留下原始字节。
- 与 DEBUG 摘要使用相同的 `Received:` / `Response:` 标签，由级别区分。

## 6. 轮转日志文件（RotatedLogFile）

| 字段 | 类型 | 说明 |
|------|------|------|
| `base_path` | 路径 | 当前活跃日志文件路径，如 `/opt/stunserver/log/stunserver.log` |
| `date` | `YYYY-MM-DD` | 日志内容所属本地日期 |
| `sequence` | u32 | 当天序号，从 1 开始，不补零 |
| `full_path` | 路径 | `{base_path}.{date}.{sequence}` |

**验证规则**
- 生成候选名称后若已存在，序号 +1 重试，禁止覆盖。
- 大小轮转使用当天日期；跨天轮转使用被归档内容所属的 `current_date`。
- 跨天后新一天首次轮转的序号从 1 开始。
- 清理逻辑继续按活跃文件名前缀匹配所有轮转文件（含历史无序号文件），按修改时间由旧到新删除，超过保留上限的部分被移除。

**命名示例**

```text
stunserver.log                  # 当前活跃日志
stunserver.log.2026-10-31.1     # 当天第 1 个轮转文件
stunserver.log.2026-10-31.2     # 当天第 2 个轮转文件
stunserver.log.2026-11-01.1     # 次日第 1 个轮转文件
```

## 7. 请求处理状态（RequestLogFlow）

```text
收到 UDP 数据
  ├─ TRACE: 记录 Received 原始字节
  ├─ 解码成功且为 Binding Request
  │    ├─ DEBUG: 记录 Received 摘要
  │    ├─ 构造响应
  │    ├─ DEBUG: 记录 Response 摘要
  │    ├─ TRACE: 记录 Response 原始字节
  │    └─ 发送；失败时 WARN + 错误计数
  └─ 解码失败或类型不符
       ├─ DEBUG: 记录 Invalid STUN request + 来源地址
       └─ 错误计数 +1
```
