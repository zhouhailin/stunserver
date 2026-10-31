# Implementation Plan: 日志 TRACE 分级与轮转命名优化

**Branch**: `006-log-trace-level` | **Date**: 2026-10-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/006-log-trace-level/spec.md`

## Summary

在现有日志体系上增加 TRACE 详细度：原始报文十六进制输出下沉到 TRACE，DEBUG 改为输出解析后的消息摘要（类型、事务 ID、属性及映射地址）；命令行新增 `--trace` 并保持 `--debug`；日志轮转文件统一命名为 `{前缀}.log.{YYYY-MM-DD}.{N}`（N 不补零、按天从 1 递增、不覆盖）。通过保留未知属性的原始类型码，保证摘要能准确展示 `0xXXXX(len=n)`。

## Technical Context

**Language/Version**: Rust stable（edition 2021）；CI 使用 stable 工具链，Docker 构建使用 Rust 1.96

**Primary Dependencies**: tokio 1.35（UDP/异步）、clap 4.4（CLI）、log 0.4（日志门面）、chrono 0.4（时间戳/轮转日期）、stun-protocol（workspace 协议库）；无新增依赖

**Storage**: 本地文件系统日志（现有 `log_dir` + `file_prefix`）

**Testing**: `cargo test`（单元测试 + `src/server/tests/integration.rs` 集成测试）、`tempfile`、release 构建后使用 `stunclient` 手工验证

**Target Platform**: Linux 服务器（UDP 3478），Rust 跨平台可运行

**Project Type**: Rust workspace：服务端 CLI 守护进程 + 协议库

**Performance Goals**: INFO/WARN/ERROR 级别无新增每请求开销；DEBUG 摘要格式化仅在 DEBUG 启用时执行；TRACE 原始报文为显式排障选项，不设为默认

**Constraints**: 严格遵守 RFC 5389；默认级别保持 INFO；不使用 `unsafe`；clippy `-D warnings`；不新增第三方依赖

**Scale/Scope**: 小型特性，预计改动服务端 4 个源文件、协议库 1 个文件及中英文 README 轮转章节

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原则 | 检查结果 | 说明 |
|------|----------|------|
| I. RFC 5389 Compliance | PASS | 不改变线上报文格式；为 `StunAttribute` 保留原始类型码可避免未知属性回编码时被写成 `0xFFFF`，提高协议保真度 |
| II. Test-Driven Development | PASS | 任务按“先写失败测试 → 实现 → 通过”排序，覆盖 CLI 优先级、摘要格式、未知属性、轮转命名与跨天场景 |
| III. Performance-First Design | PASS | 摘要与原始报文均受日志级别门控，INFO 下零额外格式化；不引入锁竞争或新分配热点 |
| IV. Code Quality Gates | PASS | 收尾执行 `cargo fmt --all -- --check`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test --all` |
| V. Simplicity and YAGNI | PASS | 复用现有自研 logger 与 `log` 宏；仅在 `server.rs` 增加私有格式化函数，不新增模块或依赖 |
| VI. CI Parity | PASS | 额外执行 `cargo build --release`，与 GitHub Actions 一致 |

**Post-Design Re-check**: PASS，无违反项，`Complexity Tracking` 无需填写。

## Project Structure

### Documentation (this feature)

```text
specs/006-log-trace-level/
├── plan.md              # 本文件
├── research.md          # Phase 0 输出
├── data-model.md        # Phase 1 输出
├── quickstart.md        # Phase 1 输出
├── contracts/           # Phase 1 输出
│   ├── cli.md
│   ├── log-output.md
│   └── log-file-naming.md
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 输出（由 /speckit-tasks 生成）
```

### Source Code (repository root)

```text
src/server/
├── main.rs              # 新增 --trace；提取 CLI 覆盖逻辑便于测试
├── config.rs            # 现有 level/validate 已支持 trace，补充覆盖测试
├── logging.rs           # 轮转命名改为 {prefix}.log.{date}.{N}
├── server.rs            # TRACE 原始报文 + DEBUG 消息摘要；解码一次复用
├── lib.rs
└── tests/
    └── integration.rs   # 现有 UDP 回环测试保持通过

crates/stun-protocol/src/
└── attributes.rs        # 保留原始属性类型码，支持未知属性展示

README.md                # 更新日志轮转命名说明
README.zh-CN.md          # 更新日志轮转命名说明
```

**Structure Decision**: 沿用现有单一 Rust workspace 结构。日志格式化作为 `server.rs` 的私有函数实现，避免为新格式化逻辑引入额外模块；协议库仅做最小字段扩展以保留未知属性类型码。

## Phase 0: Outline & Research

输出：[research.md](./research.md)

关键决定：
1. `--trace` 与 `--debug` 同时出现时 TRACE 优先，不报错。
2. TRACE 输出原始字节，DEBUG 输出解析摘要，两者不重复。
3. 摘要格式固定为 `Received: <类型> txn=<24位hex> attrs=[...]` / `Response: ...`。
4. `StunAttribute` 增加 `raw_type: u16`，未知属性展示真实类型码。
5. 轮转文件命名 `{path}.{YYYY-MM-DD}.{N}`，按天递增、不补零；跨天轮转使用被归档内容所属日期命名。
6. 请求处理路径只解码一次，响应摘要与响应字节共享同一 `StunMessage`。

## Phase 1: Design & Contracts

输出：
- [data-model.md](./data-model.md)
- [contracts/cli.md](./contracts/cli.md)
- [contracts/log-output.md](./contracts/log-output.md)
- [contracts/log-file-naming.md](./contracts/log-file-naming.md)
- [quickstart.md](./quickstart.md)

### 实施要点（供 /speckit-tasks 拆分）

1. **协议库（测试先行）**：为 `StunAttribute` 增加原始类型码字段并更新 `new/decode/encode`；测试未知属性往返保持类型码。
2. **CLI（测试先行）**：为 `Args` 增加 `--trace`；提取级别覆盖函数并测试“仅 debug / 仅 trace / 同时指定 / 都不指定”四种组合。
3. **摘要格式化（测试先行）**：在 `server.rs` 增加私有 `describe_message`/`describe_attribute`，测试请求含 SOFTWARE、响应含 XOR-MAPPED-ADDRESS（IPv4/IPv6）、未知属性、地址解码失败回退。
4. **请求处理改造**：接收后先输出 TRACE 原始字节；解码一次后输出 DEBUG 接收摘要；构造响应后输出 DEBUG 响应摘要与 TRACE 响应字节；无效请求保持 DEBUG 无效日志与错误计数。
5. **轮转命名（测试先行）**：重写 `rotate` 生成 `.{date}.{N}`；同日递增、不覆盖；跨天使用旧 `current_date` 归档；保留策略兼容历史文件名。
6. **文档与验证**：更新中英文 README 轮转命名；执行完整质量门与手工 `stunclient` 验证。

## Complexity Tracking

> 无 Constitution Check 违规项，无需填写。
