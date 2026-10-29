# Contract: Makefile Commands Interface

**Version**: 1.0  
**Date**: 2026-10-29  
**Status**: Defined

## Overview

本文档定义两个 Makefile 的命令接口契约：根目录 Makefile（开发命令）和 deploy/Makefile（部署命令）。

---

## Contract 1: Root Makefile (Development Commands)

**Location**: `/opt/stunserver/Makefile`  
**Purpose**: 提供开发者常用的构建、测试、打包命令  
**Required Permissions**: 普通用户权限（无需 root）

### Command: `make build`

**Description**: 编译 release 版本的 STUN 服务器二进制文件

**Input**:
- 源码目录：`src/`, `crates/`
- Cargo.toml, Cargo.lock

**Output**:
- 二进制文件：`target/release/stunserver`
- 退出码：0（成功）或非 0（失败）

**Side Effects**:
- 创建或更新 `target/` 目录
- 下载依赖（首次构建）

**Example**:
```bash
$ make build
cargo build --release
   Compiling stun-server v0.1.1
    Finished release [optimized] target(s) in 2m 30s
```

**Error Handling**:
- 编译错误：显示 rustc 错误信息，退出码非 0
- 依赖下载失败：显示网络错误，退出码非 0

---

### Command: `make test`

**Description**: 运行所有单元测试和集成测试

**Input**:
- 测试文件：`tests/`, `src/**/tests.rs`

**Output**:
- 测试结果摘要
- 退出码：0（所有测试通过）或非 0（有测试失败）

**Side Effects**:
- 创建测试覆盖率报告（如果配置）
- 生成测试二进制文件

**Example**:
```bash
$ make test
cargo test
   Compiling stun-server v0.1.1
    Finished test [unoptimized + debuginfo] target(s) in 1m 15s
     Running unittests (target/debug/deps/stun_server-abc123)
test result: ok. 42 passed; 0 failed; 0 ignored
     Running tests/integration_test.rs
test result: ok. 15 passed; 0 failed; 0 ignored
```

**Error Handling**:
- 测试失败：显示失败的测试用例和断言信息，退出码非 0
- 编译错误：同 `make build`

---

### Command: `make package`

**Description**: 创建分发包，包含二进制文件、配置文件、systemd 服务文件

**Input**:
- 已编译的二进制：`target/release/stunserver`
- 配置示例：`deploy/stunserver.conf.example`
- systemd 服务：`systemd/stunserver.service`
- 版本号：从 Cargo.toml 读取或环境变量 `VERSION`

**Output**:
- 分发包：`dist/stunserver-{version}.tar.gz`
- 包内容结构：
  ```
  stunserver-bin/stunserver
  stunserver-etc/stunserver.conf
  stunserver-log/
  stunserver.service
  README.md
  ```

**Side Effects**:
- 创建 `dist/` 目录
- 清理旧的 dist 内容

**Example**:
```bash
$ make package VERSION=0.1.1
Creating distribution package...
Package created: dist/stunserver-0.1.1.tar.gz
$ tar -tzf dist/stunserver-0.1.1.tar.gz
stunserver-bin/stunserver
stunserver-etc/stunserver.conf
stunserver-log/
stunserver.service
README.md
```

**Error Handling**:
- 二进制不存在：提示先运行 `make build`，退出码非 0
- 版本号未提供：使用 Cargo.toml 中的版本或 "latest"

---

### Command: `make clean`

**Description**: 清理所有构建产物

**Input**: None

**Output**:
- 删除 `target/` 目录
- 删除 `dist/` 目录
- 退出码：0

**Side Effects**:
- 下次构建需要重新编译所有依赖

**Example**:
```bash
$ make clean
cargo clean
rm -rf dist/
```

---

### Command: `make fmt`

**Description**: 格式化所有 Rust 源码

**Input**:
- 源码文件：`src/**/*.rs`, `crates/**/*.rs`

**Output**:
- 格式化后的源码
- 退出码：0

**Side Effects**:
- 修改源码文件格式

**Example**:
```bash
$ make fmt
cargo fmt --all
```

---

### Command: `make lint`

**Description**: 运行 clippy 静态分析，零警告策略

**Input**:
- 源码文件

**Output**:
- clippy 警告或错误信息
- 退出码：0（无警告）或非 0（有警告）

**Example**:
```bash
$ make lint
cargo clippy -- -D warnings
    Checking stun-server v0.1.1
    Finished dev [unoptimized + debuginfo] target(s) in 0.50s
```

**Error Handling**:
- 发现警告：显示警告详情，退出码非 0

---

### Command: `make help`

**Description**: 显示所有可用命令的帮助信息

**Input**: None

**Output**:
- 命令列表和简要说明
- 退出码：0

**Example**:
```bash
$ make help
Available commands:
  make build    - Build release binary
  make test     - Run all tests
  make package  - Create distribution package
  make clean    - Clean build artifacts
  make fmt      - Format code
  make lint     - Run clippy
  make help     - Show this help message
```

---

## Contract 2: deploy/Makefile (Deployment Commands)

**Location**: `/opt/stunserver/deploy/Makefile`  
**Purpose**: 提供生产环境的安装、卸载、服务管理命令  
**Required Permissions**: root 权限（通过 sudo）

### Command: `make install`

**Description**: 安装 STUN 服务器到 /opt/stunserver，配置并启用 systemd 服务

**Preconditions**:
- 必须以 root 权限运行（或使用 sudo）
- 系统必须支持 systemd（`command -v systemctl` 存在）
- 二进制文件必须已编译（`target/release/stunserver` 存在）

**Input**:
- 二进制文件：`../target/release/stunserver`
- 配置示例：`stunserver.conf.example`
- systemd 服务：`../systemd/stunserver.service`

**Output**:
- 安装目录：`/opt/stunserver/{bin,etc,log}`
- systemd 服务已启用
- stunserver 系统用户已创建
- 退出码：0（成功）或非 0（失败）

**Side Effects**:
- 创建系统用户 `stunserver`（如果不存在）
- 复制文件到 /opt/stunserver
- 设置文件权限
- 重载 systemd daemon
- 启用 stunserver 服务

**Example**:
```bash
$ sudo make install
Installing STUN server to /opt/stunserver...
Installation complete. Service enabled.
$ systemctl status stunserver
● stunserver.service - STUN Server
   Loaded: loaded (/etc/systemd/system/stunserver.service; enabled)
   Active: inactive (dead)
```

**Error Handling**:
- 非 root 用户：提示 "Error: root privileges required. Use sudo."，退出码 1
- systemd 不存在：提示 "Error: systemd not found. This installer only supports systemd-based systems."，退出码 1
- 安装目录已存在：备份旧版本或提示用户确认覆盖
- 文件复制失败：显示错误信息，回滚已安装的文件，退出码 1

**Idempotency**:
- 多次运行 `make install` 是安全的
- 如果已安装，会更新文件并重新启用服务

---

### Command: `make uninstall`

**Description**: 完全卸载 STUN 服务器，停止并禁用 systemd 服务，删除所有文件

**Preconditions**:
- 必须以 root 权限运行

**Input**: None

**Output**:
- systemd 服务已停止并禁用
- /opt/stunserver 目录已删除
- stunserver 系统用户已删除（可选）
- 退出码：0

**Side Effects**:
- 停止正在运行的服务
- 删除所有安装文件（包括配置和日志）
- 删除系统用户

**Example**:
```bash
$ sudo make uninstall
Uninstalling STUN server...
Uninstallation complete.
```

**Error Handling**:
- 服务未安装：警告但不报错，继续清理
- 文件删除失败：显示错误，继续尝试删除其他文件
- 用户删除失败：警告但不报错（用户可能被其他进程使用）

**Data Loss Warning**:
- ⚠️ 所有日志文件和配置文件将被永久删除
- 建议卸载前备份重要数据

---

### Command: `make start`

**Description**: 启动 STUN 服务器服务

**Preconditions**:
- 服务必须已安装（`make install`）
- 必须以 root 权限运行

**Input**: None

**Output**:
- 服务状态变为 active (running)
- 退出码：0

**Example**:
```bash
$ sudo make start
Service started.
$ sudo make status
● stunserver.service - STUN Server
   Active: active (running) since Mon 2026-10-29 10:00:00 UTC
```

**Error Handling**:
- 服务未安装：提示 "Error: Service not installed. Run 'make install' first."，退出码 1
- 启动失败：显示 journalctl 日志，退出码 1
- 端口被占用：显示错误信息，提示检查配置

---

### Command: `make stop`

**Description**: 优雅停止 STUN 服务器服务

**Preconditions**:
- 服务必须正在运行

**Input**: None

**Output**:
- 服务状态变为 inactive (dead)
- 所有连接正常关闭
- 退出码：0

**Example**:
```bash
$ sudo make stop
Service stopped.
```

**Error Handling**:
- 服务未运行：警告但不报错，退出码 0
- 停止超时：强制杀死进程，显示警告

---

### Command: `make restart`

**Description**: 重启 STUN 服务器服务（先停止后启动）

**Preconditions**:
- 服务必须已安装

**Input**: None

**Output**:
- 服务重启完成
- 配置重新加载
- 退出码：0

**Example**:
```bash
$ sudo make restart
Service restarted.
```

**Error Handling**:
- 停止失败：继续尝试启动，显示警告
- 启动失败：显示错误信息，退出码 1

---

### Command: `make status`

**Description**: 显示 STUN 服务器服务的当前状态

**Preconditions**: None

**Input**: None

**Output**:
- systemd 服务状态信息
- 退出码：0（运行中）或 3（未运行）或 4（未知）

**Example**:
```bash
$ sudo make status
● stunserver.service - STUN Server - Session Traversal Utilities for NAT
   Loaded: loaded (/etc/systemd/system/stunserver.service; enabled)
   Active: active (running) since Mon 2026-10-29 10:00:00 UTC
  Process: 12345 ExecStart=/opt/stunserver/bin/stunserver --config /opt/stunserver/etc/stunserver.conf
 Main PID: 12345 (stunserver)
    Tasks: 8 (limit: 4915)
   Memory: 12.5M
   CGroup: /system.slice/stunserver.service
           └─12345 /opt/stunserver/bin/stunserver --config /opt/stunserver/etc/stunserver.conf
```

---

### Command: `make help`

**Description**: 显示所有部署命令的帮助信息

**Example**:
```bash
$ sudo make help
Available commands (requires root):
  make install   - Install and enable service
  make uninstall - Stop, disable, and remove service
  make start     - Start service
  make stop      - Stop service
  make restart   - Restart service
  make status    - Show service status
  make help      - Show this help message
```

---

## Common Constraints

### Permission Model

| Command      | Required Permission | Elevation Method |
|--------------|---------------------|------------------|
| build        | User                | N/A              |
| test         | User                | N/A              |
| package      | User                | N/A              |
| clean        | User                | N/A              |
| fmt          | User                | N/A              |
| lint         | User                | N/A              |
| help         | User                | N/A              |
| install      | Root                | sudo             |
| uninstall    | Root                | sudo             |
| start        | Root                | sudo             |
| stop         | Root                | sudo             |
| restart      | Root                | sudo             |
| status       | Root                | sudo             |

### Exit Codes

| Code | Meaning                          |
|------|----------------------------------|
| 0    | 成功                             |
| 1    | 一般错误（权限、前置条件不满足） |
| 2    | 用法错误（无效参数）             |
| 126  | 命令不可执行                     |
| 127  | 命令未找到                       |
| 128+ | 信号终止（128 + 信号编号）       |

### Logging

所有命令执行过程中的关键操作应输出到 stdout，错误信息输出到 stderr。

**Install/Uninstall 命令日志示例**:
```
[INFO] Checking prerequisites...
[INFO] systemd detected: /bin/systemctl
[INFO] Creating directory: /opt/stunserver/bin
[INFO] Copying binary: stunserver -> /opt/stunserver/bin/stunserver
[INFO] Setting permissions: 755 /opt/stunserver/bin/stunserver
[INFO] Creating system user: stunserver
[INFO] Enabling service: stunserver
[SUCCESS] Installation complete
```

---

## Versioning

本契约遵循语义化版本（Semantic Versioning）：
- MAJOR: 不兼容的 API 变更
- MINOR: 向后兼容的功能新增
- PATCH: 向后兼容的问题修正

**Current Version**: 1.0.0
