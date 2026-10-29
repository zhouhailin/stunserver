# Implementation Plan: 打包优化与 Docker 镜像支持

**Branch**: `003-packaging-docker-support` | **Date**: 2026-10-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/003-packaging-docker-support/spec.md`

## Summary

为 STUN 服务器实现标准化打包和容器化部署支持。技术方法包括：创建分离的 Makefile 结构（根目录用于开发，deploy/ 用于部署），实现 /opt/stunserver 集中目录结构（bin/log/etc），提供 systemd 服务集成，构建 Docker 镜像支持，以及实现应用内置日志轮转功能。

## Technical Context

**Language/Version**: Rust 1.75+ (edition 2021)

**Primary Dependencies**: 
- tokio 1.35 (async runtime)
- clap 4.4 (CLI)
- serde 1.0 + serde_yaml 0.9 (configuration)
- log 0.4 + env_logger 0.10 (logging)
- anyhow 1.0 (error handling)

**Storage**: 文件系统（配置文件 /opt/stunserver/etc/stunserver.conf，日志文件 /opt/stunserver/log/）

**Testing**: cargo test (单元测试 + 集成测试)

**Target Platform**: Linux (systemd 支持，Ubuntu 20.04+, CentOS 7+, Debian 10+)

**Project Type**: CLI 工具 / 网络服务器

**Performance Goals**: 
- 安装时间 < 5 分钟
- Docker 镜像构建时间 < 10 分钟
- 容器启动后服务可用时间 < 30 秒
- Docker 镜像大小 < 200MB

**Constraints**: 
- 仅支持 systemd 初始化系统
- 需要 root 权限安装
- 日志文件自动轮转（100MB 大小，30 天保留）
- 单一配置文件（TOML 格式）

**Scale/Scope**: 
- 4 个用户故事（P1-P4）
- 16 个功能需求
- 8 个成功标准
- 5 个边缘情况

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Gate 1: RFC 5389 Compliance
✅ **PASS** - 本功能不涉及 STUN 协议实现，仅涉及打包和部署，不影响协议合规性。

### Gate 2: Test-Driven Development (NON-NEGOTIABLE)
✅ **PASS** - 所有新增代码（日志轮转逻辑、配置解析等）必须遵循 TDD：
- 先编写失败的测试
- 实现最小代码使测试通过
- 重构同时保持测试绿色
- 测试覆盖率不得降低

### Gate 3: Performance-First Design
✅ **PASS** - 设计决策考虑性能影响：
- 日志轮转使用异步 I/O 避免阻塞主线程
- 配置文件解析仅在启动时执行一次
- Docker 镜像使用多阶段构建减小体积

### Gate 4: Code Quality Gates
✅ **PASS** - 所有代码必须通过：
- `cargo fmt` — 格式化
- `cargo clippy -- -D warnings` — 零警告
- `cargo test` — 所有测试通过
- 无 `unsafe` 块（除非有明确理由）

### Gate 5: Simplicity and YAGNI
✅ **PASS** - 设计遵循最简原则：
- 单一配置文件而非多个
- 应用内置日志轮转而非引入外部依赖
- 分离 Makefile 而非复杂的多目标文件
- 仅支持 systemd 而非多 init 系统

**Pre-Design Gate Result**: ✅ ALL GATES PASS — 可进入 Phase 0

## Project Structure

### Documentation (this feature)

```text
specs/003-packaging-docker-support/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── makefile-commands.md
│   └── docker-interface.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
/opt/stunserver/
├── Makefile                    # 根目录 Makefile（开发命令：build/test/package/clean）
├── deploy/
│   ├── Makefile                # 部署 Makefile（install/uninstall/start/stop/restart）
│   └── stunserver.conf.example # 示例配置文件
├── docker/
│   ├── Dockerfile              # 多阶段构建 Dockerfile
│   └── docker-entrypoint.sh    # 容器入口脚本
├── systemd/
│   └── stunserver.service      # systemd 服务配置（更新路径和 URL）
├── src/
│   ├── server/
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── config.rs       # 配置解析（支持 TOML）
│   │   │   └── logging.rs      # 日志轮转逻辑
│   │   └── Cargo.toml
│   └── ...
└── tests/
    ├── integration/
    └── ...
```

**Structure Decision**: 采用单项目结构，新增 deploy/ 和 docker/ 目录分别存放部署脚本和 Docker 相关文件。根目录 Makefile 用于开发，deploy/Makefile 用于生产部署。

## Complexity Tracking

> **无宪法违规需要证明**

所有设计决策均符合宪法原则，无需额外复杂性证明。

## Phase 0: Research & Unknowns Resolution

详见 [research.md](./research.md)

## Phase 1: Design & Contracts

详见：
- [data-model.md](./data-model.md) — 数据模型和实体定义
- [contracts/](./contracts/) — 接口契约定义
- [quickstart.md](./quickstart.md) — 快速验证指南

## Next Steps

Phase 0 和 Phase 1 完成后，运行 `/speckit-tasks` 生成详细的任务分解（tasks.md）。
