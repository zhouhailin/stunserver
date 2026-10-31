# CLI Contract: 日志级别开关

**Feature**: `006-log-trace-level`

## 参数

| 参数 | 类型 | 说明 |
|------|------|------|
| `--trace` | flag | 将日志级别设为 TRACE，输出原始报文与 DEBUG 摘要 |
| `--debug` | flag | 将日志级别设为 DEBUG，输出解析后的消息摘要（不含原始报文） |
| `-q, --quiet` | flag | 保持现有行为：抑制启动信息行 |

其余参数（`-c/--config`、`-l/--listen`）行为不变。

## 级别优先级

从高到低：

```text
--trace  >  --debug  >  配置文件 logging.level  >  默认 info
```

- `--trace --debug` 同时出现：不报错，按 TRACE 运行。
- 未指定任何级别开关：完全沿用配置文件；无配置文件时为 INFO。
- 配置文件 `level = "trace"` 与 `--trace` 效果一致。

## 帮助文本要求

`stunserver --help` 必须包含两个开关及其含义：

```text
--debug    Enable debug logging (prints parsed message summaries)
--trace    Enable trace logging (prints raw packet bytes; implies debug)
```

## 退出码

本特性不改变现有退出码：
- `0`：正常退出路径
- `2`：绑定/权限类网络错误
- `3`：其他运行时错误
- `130`：优雅停止

## 兼容性

- 现有脚本仅使用 `--debug` 时行为兼容：仍输出 DEBUG 摘要，但原始字节不再出现（原始字节改由 `--trace` 输出）。
- 配置文件中的 `trace` 级别此前已被接受，本次使其产生实际的 TRACE 输出。
