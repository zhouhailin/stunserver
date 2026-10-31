# Log Output Contract: 控制台与文件日志

**Feature**: `006-log-trace-level`

## 通用格式

```text
<timestamp> [<LEVEL>] <message>
```

- `timestamp`：`YYYY-MM-DDTHH:MM:SS.sss`（本地时间，毫秒精度）
- `LEVEL`：`TRACE` / `DEBUG` / `INFO` / `WARN` / `ERROR`
- 控制台与文件使用相同格式；控制台写 stderr，文件追加写入活跃日志

## DEBUG 记录（每个合法请求至少 3 条）

```text
2026-10-31T14:52:08.405 [DEBUG] Request from 127.0.0.1:45657 (20 bytes)
2026-10-31T14:52:08.405 [DEBUG] Received: Binding Request txn=09e96d1c0e8ae31800000000 attrs=[SOFTWARE="test-client"]
2026-10-31T14:52:08.405 [DEBUG] Response: Binding Success Response txn=09e96d1c0e8ae31800000000 attrs=[XOR-MAPPED-ADDRESS=127.0.0.1:45657]
```

规则：
- `Request from` 行保留来源地址与字节数（FR-014）
- 摘要包含消息类型、事务 ID、属性列表（FR-006/FR-007）
- 空属性：`attrs=[]`
- 未知属性：`0xABCD(len=4)`
- 地址无法解码：`XOR-MAPPED-ADDRESS(len=8)`
- `SOFTWARE` 非法 UTF-8 使用替换字符，不报错

## TRACE 记录（仅 `--trace` 或配置 trace）

```text
2026-10-31T14:52:08.405 [TRACE] Received: [00, 01, 00, 00, 21, 12, a4, 42, ...]
2026-10-31T14:52:08.405 [TRACE] Response: [01, 01, 00, 0c, 21, 12, a4, 42, ...]
```

规则：
- 完整原始字节，小写十六进制，字节间 `, `，外层 `[...]`
- 接收报文在解码前记录，因此无效报文也有 TRACE 记录
- TRACE 开启时同时出现 DEBUG 记录（级别包含关系）
- DEBUG 及以下级别不得出现 `Received:`/`Response:` 的十六进制字节内容

## 无效请求

```text
2026-10-31T14:52:08.405 [DEBUG] Invalid STUN request from 127.0.0.1:45657
```

同时：错误计数 +1，服务继续运行；TRACE 下另有原始字节记录。

## 日志标签

- DEBUG 摘要与 TRACE 原始字节均使用 `Received:` / `Response:` 前缀
- 唯一区分方式为级别和内容形态（摘要 vs 十六进制列表）
