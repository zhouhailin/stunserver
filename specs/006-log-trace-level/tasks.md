# Tasks: 日志 TRACE 分级与轮转命名优化

**Input**: Design documents from `specs/006-log-trace-level/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: 本特性遵循项目宪法 Principle II（TDD，不可协商），因此每个用户故事均包含测试任务，且必须先写失败测试再实现。

**Organization**: 任务按用户故事分组（US1/P1、US2/P2、US3/P3），每个故事可独立验证。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行执行（不同文件、无未完成依赖）
- **[Story]**: 所属用户故事（US1/US2/US3）
- 描述中包含精确文件路径

## Path Conventions

Rust workspace：
- 服务端 crate：`src/server/`
- 协议库 crate：`crates/stun-protocol/`
- 服务端集成测试：`src/server/tests/`

---

## Phase 1: Setup

**Purpose**: 确认基线状态

- [x] T001 在仓库根目录（`Cargo.toml`）运行基线验证：`cargo build --all` 与 `cargo test --all`，确认当前分支 `006-log-trace-level` 上既有测试全部通过并记录结果

**Checkpoint**: 基线可构建、可测试

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 跨故事共享前置实现

本特性无跨故事共享的阻塞性实现：三个用户故事分别改动独立关注点（CLI/请求日志、消息摘要、轮转命名）。US1 与 US2 因共同修改 `src/server/server.rs` 与 `src/server/tests/logging_levels.rs`，需按优先级顺序实施；US3 可与 US1/US2 并行。

**Checkpoint**: 无阻塞项，可直接进入 Story 阶段

---

## Phase 3: User Story 1 - 通过命令行开启 TRACE 日志 (Priority: P1) 🎯 MVP

**Goal**: 新增 `--trace` 参数；原始报文十六进制日志仅在 TRACE 输出，`--debug` 不再输出原始字节。

**Independent Test**: 分别以 `--trace` 与 `--debug` 启动，发送一次 Binding 请求；TRACE 日志包含 `Received:`/`Response:` 原始字节，DEBUG 日志不包含。

### Tests for User Story 1 ⚠️ 先写测试并确认失败

- [x] T002 [P] [US1] 在 `src/server/main.rs` 的 `#[cfg(test)]` 模块增加失败测试：用 `Args::try_parse_from` 与待实现的级别覆盖函数验证四种组合（仅 `--debug` → debug；仅 `--trace` → trace；同时指定 → trace；都不指定 → 保持配置级别），契约见 `specs/006-log-trace-level/contracts/cli.md`
- [x] T003 [P] [US1] 新建 `src/server/tests/logging_levels.rs` 端到端失败测试：通过 `env!("CARGO_BIN_EXE_stunserver")` 启动二进制（`--listen 127.0.0.1:<临时端口>`），发送 Binding 请求并捕获 stderr；断言 `--trace` 下存在 `[TRACE] Received: [` 与 `[TRACE] Response: [` 且同时含 DEBUG 记录，`--debug` 下不存在原始字节记录，契约见 `contracts/log-output.md`

### Implementation for User Story 1

- [x] T004 [US1] 在 `src/server/main.rs` 为 `Args` 增加 `--trace` 布尔参数与帮助文本，提取级别覆盖函数并实现优先级 `trace > debug > 配置 > info`；不新增依赖（依赖 T002）
- [x] T005 [US1] 在 `src/server/server.rs` 将接收/响应原始字节日志从 `debug!` 改为 `trace!`（保留 `Received:`/`Response:` 标签与 `{:02x?}` 格式），保留 `Request from {地址} ({字节数} bytes)` 为 DEBUG，并补充 `trace` 导入（依赖 T003）

**Checkpoint**: `--trace` 可输出原始报文，`--debug` 无原始字节；US1 可独立验收

---

## Phase 4: User Story 2 - DEBUG 级别展示可读的报文内容 (Priority: P2)

**Goal**: DEBUG 输出 `Received:`/`Response:` 消息摘要（类型、事务 ID、属性及映射地址）；未知属性显示真实类型码；无效请求保持 DEBUG 记录；请求只解码一次。

**Independent Test**: 以 `--debug` 启动并发送 Binding 请求，日志含接收摘要与响应摘要（含 `XOR-MAPPED-ADDRESS=<ip>:<port>`），且无十六进制原始字节。

### Tests for User Story 2 ⚠️ 先写测试并确认失败

- [x] T006 [P] [US2] 在 `crates/stun-protocol/tests/protocol.rs` 增加失败测试：构造并解析未知属性，断言保留线上原始类型码且 `encode()` 后类型码不变（避免被写成 `0xFFFF`），字段约束见 `data-model.md` AttributeSummary.raw_type
- [x] T007 [P] [US2] 在 `src/server/server.rs` 的 `#[cfg(test)]` 模块为待实现的 `describe_message`/`describe_attribute` 增加失败测试：请求含 `SOFTWARE`、响应含 `XOR-MAPPED-ADDRESS`（IPv4 与 IPv6）、未知属性输出 `0xXXXX(len=n)`、地址解码失败回退 `NAME(len=n)`、空属性输出 `attrs=[]`，格式见 `contracts/log-output.md`

### Implementation for User Story 2

- [x] T008 [US2] 在 `crates/stun-protocol/src/attributes.rs` 为 `StunAttribute` 增加 `raw_type: u16`：`new()` 由枚举初始化，`decode()` 保存原始类型码，`encode()` 写回 `raw_type`（依赖 T006）
- [x] T009 [US2] 在 `src/server/server.rs` 实现私有 `describe_message` 与 `describe_attribute`：消息类型名、24 位小写十六进制事务 ID、按报文顺序输出属性；地址类属性解码为 `ip:port`，`SOFTWARE` 使用 UTF-8 lossy，未知属性使用 `raw_type` 输出 `0xXXXX(len=n)`（依赖 T007、T008）
- [x] T010 [US2] 在 `src/server/server.rs` 重构请求路径：提取 `build_response_message(&StunMessage, SocketAddr) -> StunMessage`，处理循环只解码一次；合法请求在 DEBUG 输出接收摘要与响应摘要，响应发送前输出 TRACE 原始字节；无效请求保持 `debug!("Invalid STUN request from {}", addr)` 并计入错误；保留公开 `build_response` 薄包装兼容现有测试（依赖 T005、T009）
- [x] T011 [US2] 扩展 `src/server/tests/logging_levels.rs`：断言 `--debug` 下每个成功请求有 `Request from`、`Received:` 摘要、`Response:` 摘要各一条，摘要含 `txn=` 与 `XOR-MAPPED-ADDRESS=`，且原始字节出现次数为 0（依赖 T003、T010）

**Checkpoint**: US1 与 US2 同时可用；DEBUG 可读、TRACE 含原始字节

---

## Phase 5: User Story 3 - 轮转日志文件名包含日期与序号 (Priority: P3)

**Goal**: 轮转文件统一命名 `{前缀}.log.{YYYY-MM-DD}.{N}`，N 不补零、按天从 1 递增、不覆盖；跨天使用内容所属旧日期归档。

**Independent Test**: 触发同一天多次轮转与跨天轮转，检查文件名序号递增、不覆盖、次日从 1 开始；历史无序号文件仍参与清理。

### Tests for User Story 3 ⚠️ 先写测试并确认失败

- [x] T012 [US3] 在 `src/server/logging.rs` 增加失败测试：首次轮转为 `{prefix}.log.{当天}.1`；已存在 `.1` 时生成 `.2` 且原文件未被覆盖；跨天使用旧 `current_date` 归档且新一天从 1 开始；混有历史 `{prefix}.log.2026-10-29` 时清理行为不变，规则见 `contracts/log-file-naming.md`

### Implementation for User Story 3

- [x] T013 [US3] 在 `src/server/logging.rs` 重写 `rotate` 命名：从 1 开始扫描 `{active_path}.{date}.{N}` 选择首个不存在名称；大小轮转使用当天日期，跨天轮转使用旧 `current_date` 归档后再更新日期；保持 `prune_old_files` 对历史文件名的兼容（依赖 T012）
- [x] T014 [P] [US3] 更新 `README.md` 与 `README.zh-CN.md` 的日志轮转章节，将示例文件名改为 `stunserver.log.YYYY-MM-DD.N` 并说明按天递增（与 T013 并行）

**Checkpoint**: 轮转命名符合契约，历史文件与保留策略不受影响

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: 跨故事验证与质量门

- [x] T015 [P] 审查 `src/server/server.rs` 日志门控：INFO 下无额外格式化；DEBUG 不输出十六进制原始字节；TRACE 同时包含摘要与原始字节；确认 `build_response` 公共 API 未破坏
- [x] T016 在仓库根目录（`Cargo.toml`）执行完整质量门：`cargo fmt --all -- --check`、`cargo clippy --all-targets --all-features -- -D warnings`、`cargo test --all`、`cargo build --release`
- [x] T017 按 `specs/006-log-trace-level/quickstart.md` 执行场景 1–5，记录实际输出并确认 SC-001 ~ SC-005

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 无依赖，立即开始
- **Foundational (Phase 2)**: 无阻塞任务
- **User Stories**: US1 → US2 顺序实施（共享 `server.rs` 与 E2E 测试文件）；US3 可并行
- **Polish (Phase 6)**: 依赖全部故事完成

### User Story Dependencies

- **US1 (P1)**: 无故事依赖
- **US2 (P2)**: 依赖 US1（T005 的 TRACE 改造与 T003 的 E2E 测试骨架）
- **US3 (P3)**: 独立于 US1/US2（仅 `logging.rs` 与 README）

### Within Each User Story

- 测试先写并确认失败（TDD，宪法 II）
- 协议/格式化基础实现先于请求路径集成
- 故事内测试通过后再进入下一优先级

### Parallel Opportunities

- 同一故事的测试任务并行：T002 ∥ T003；T006 ∥ T007
- US1 实现：T004（`main.rs`）与 T005（`server.rs`）可在测试写好后并行
- US3：T014（README）可与 T013 并行
- US3 整体可与 US1/US2 并行（不同文件）

---

## Parallel Example: User Story 1

```bash
# 先并行编写两个失败测试：
Task: "T002 CLI 优先级测试 in src/server/main.rs"
Task: "T003 E2E 日志级别测试 in src/server/tests/logging_levels.rs"

# 测试确认失败后并行实现：
Task: "T004 --trace 参数与级别覆盖 in src/server/main.rs"
Task: "T005 原始报文改为 trace! in src/server/server.rs"
```

## Parallel Example: User Story 2

```bash
# 并行编写失败测试：
Task: "T006 未知属性类型码保留测试 in crates/stun-protocol/tests/protocol.rs"
Task: "T007 摘要格式化测试 in src/server/server.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. 完成 T001 基线
2. 完成 US1（T002–T005）
3. 独立验证 `--trace`/`--debug` 行为
4. US1 单独即为可发布切片：`--trace` 提供原始报文排障能力

> 注意：US1 单独上线会使 `--debug` 暂时失去报文内容。建议按 P1 → P2 连续交付，使“TRACE 原始字节 / DEBUG 摘要”的分工完整生效。

### Incremental Delivery

1. US1：命令行 TRACE 开关 + 原始字节下沉
2. US2：DEBUG 可读摘要（与 US1 组成完整日志分级）
3. US3：轮转命名规范化（可与上述并行）
4. Polish：质量门与 quickstart 验证

### Parallel Team Strategy

- 开发者 A：US1 → US2（同一文件链路，顺序实施）
- 开发者 B：US3（`logging.rs` + README）
- 完成后合并执行 T015–T017

---

## Notes

- TDD 强制：每个测试任务必须在实现前运行并确认因正确原因失败
- `[P]` 仅用于不同文件且无未完成依赖的任务
- 测试临时端口需通过绑定 `127.0.0.1:0` 获取后再释放，避免固定端口冲突；E2E 测试须带超时与子进程清理
- 每个任务或逻辑组完成后按 Conventional Commits 提交
