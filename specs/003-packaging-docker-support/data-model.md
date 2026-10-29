# Phase 1: Data Model & Entity Definitions

**Date**: 2026-10-29  
**Status**: ✅ Complete

## Overview

本文档定义打包优化与 Docker 镜像支持功能的核心数据模型和实体结构。

## Entity 1: Configuration File (stunserver.conf)

### Schema Definition

```toml
# /opt/stunserver/etc/stunserver.conf

[server]
# 监听地址（IP 或主机名）
listen_address = "0.0.0.0"

# STUN 服务端口（UDP 和 TCP）
port = 3478

# 工作线程数（0 = 自动检测 CPU 核心数）
worker_threads = 0

# 最大并发连接数
max_connections = 10000

[logging]
# 日志级别：trace, debug, info, warn, error
level = "info"

# 日志目录
log_dir = "/opt/stunserver/log"

# 日志文件名前缀
file_prefix = "stunserver"

# 单个日志文件最大大小（MB）
max_file_size_mb = 100

# 日志保留天数
retention_days = 30

# 是否启用控制台输出
enable_console = true

[network]
# UDP 接收缓冲区大小（字节）
udp_recv_buffer_size = 1048576

# UDP 发送缓冲区大小（字节）
udp_send_buffer_size = 1048576

# TCP 连接超时（秒）
tcp_connection_timeout_sec = 30

# TCP 空闲超时（秒）
tcp_idle_timeout_sec = 300

[security]
# 是否启用 TLS（未来扩展）
enable_tls = false

# TLS 证书路径（未来扩展）
# tls_cert_path = "/opt/stunserver/etc/cert.pem"

# TLS 私钥路径（未来扩展）
# tls_key_path = "/opt/stunserver/etc/key.pem"
```

### Validation Rules

| Field                      | Type    | Required | Default         | Validation Constraints                    |
|----------------------------|---------|----------|-----------------|-------------------------------------------|
| server.listen_address      | String  | Yes      | "0.0.0.0"       | 有效的 IP 地址或主机名                    |
| server.port                | Integer | Yes      | 3478            | 范围：1-65535                             |
| server.worker_threads      | Integer | Yes      | 0               | 范围：0-256（0 表示自动）                 |
| server.max_connections     | Integer | Yes      | 10000           | 范围：1-1000000                           |
| logging.level              | String  | Yes      | "info"          | 枚举：trace/debug/info/warn/error         |
| logging.log_dir            | String  | Yes      | "/opt/stunserver/log" | 有效的目录路径                        |
| logging.max_file_size_mb   | Integer | Yes      | 100             | 范围：1-10000                             |
| logging.retention_days     | Integer | Yes      | 30              | 范围：1-365                               |
| network.udp_recv_buffer_size | Integer | Yes    | 1048576         | 范围：4096-104857600                      |
| network.tcp_connection_timeout_sec | Integer | Yes | 30          | 范围：1-3600                              |
| security.enable_tls        | Boolean | Yes      | false           | N/A                                       |

### Rust Data Structure

```rust
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub logging: LoggingConfig,
    pub network: NetworkConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub listen_address: String,
    pub port: u16,
    pub worker_threads: usize,
    pub max_connections: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub log_dir: PathBuf,
    pub file_prefix: String,
    pub max_file_size_mb: u64,
    pub retention_days: u32,
    pub enable_console: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NetworkConfig {
    pub udp_recv_buffer_size: usize,
    pub udp_send_buffer_size: usize,
    pub tcp_connection_timeout_sec: u64,
    pub tcp_idle_timeout_sec: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SecurityConfig {
    pub enable_tls: bool,
    pub tls_cert_path: Option<PathBuf>,
    pub tls_key_path: Option<PathBuf>,
}
```

---

## Entity 2: Directory Structure

### Installation Layout

```
/opt/stunserver/
├── bin/
│   └── stunserver              # 主二进制文件（release build）
├── etc/
│   ├── stunserver.conf         # 主配置文件
│   ├── stunserver.conf.example # 示例配置（安装时复制为 stunserver.conf）
│   └── logrotate.conf          # 可选：logrotate 配置（如果使用外部轮转）
├── log/
│   ├── stunserver.log          # 当前日志文件
│   ├── stunserver.log.1        # 轮转日志 1
│   ├── stunserver.log.2        # 轮转日志 2
│   └── ...                     # 更多历史日志
└── README.md                   # 安装说明（可选）
```

### Permissions

| Path                    | Owner      | Group      | Permissions | Notes                          |
|-------------------------|------------|------------|-------------|--------------------------------|
| /opt/stunserver/        | stunserver | stunserver | 755         | 安装根目录                     |
| /opt/stunserver/bin/    | stunserver | stunserver | 755         | 二进制目录                     |
| /opt/stunserver/bin/stunserver | stunserver | stunserver | 755  | 可执行文件                     |
| /opt/stunserver/etc/    | stunserver | stunserver | 750         | 配置目录（限制访问）           |
| /opt/stunserver/etc/*.conf | stunserver | stunserver | 640      | 配置文件（仅 owner 可写）      |
| /opt/stunserver/log/    | stunserver | stunserver | 750         | 日志目录                       |
| /opt/stunserver/log/*.log | stunserver | stunserver | 640       | 日志文件                       |

### Lifecycle States

```
[Not Installed] --install--> [Installed, Stopped]
[Installed, Stopped] --start--> [Installed, Running]
[Installed, Running] --stop--> [Installed, Stopped]
[Installed, Running] --restart--> [Installed, Running]
[Installed, *] --uninstall--> [Not Installed]
```

---

## Entity 3: Log File Management

### File Naming Convention

```
{file_prefix}.log           # 当前活跃日志
{file_prefix}.log.{N}       # 轮转后的历史日志（N = 1, 2, 3, ...）
```

示例：
- stunserver.log（当前）
- stunserver.log.1（最近轮转）
- stunserver.log.2（次近轮转）
- ...

### Rotation Triggers

日志轮转在以下条件下触发（满足任一即可）：

1. **大小触发**: 当前日志文件大小 >= max_file_size_mb (100MB)
2. **时间触发**: 日期变更（午夜 00:00）

### Rotation Algorithm

```
1. 检查是否需要轮转
   if current_file.size >= max_size OR current_date > last_rotation_date:
       proceed to rotation

2. 轮转当前文件
   close(current_file)
   rename("stunserver.log", "stunserver.log.1")
   # 如果存在旧轮转文件，依次递增编号
   for i in [max_index..1]:
       if exists("stunserver.log.{i}"):
           rename("stunserver.log.{i}", "stunserver.log.{i+1}")
   
3. 创建新日志文件
   current_file = open("stunserver.log", CREATE | WRITE)

4. 清理过期日志
   for each file in log_dir matching "stunserver.log.*":
       if file.mtime < (now - retention_days):
           delete(file)
```

### State Transitions

```
[Active Log] --size>=100MB--> [Rotated Log .1]
[Active Log] --date change--> [Rotated Log .1]
[Rotated Log .N] --cleanup--> [Deleted] (if age > 30 days)
```

---

## Entity 4: Docker Image Layers

### Multi-Stage Build Structure

```
Stage 1: Builder (rust:1.75-alpine)
├── Layer 1: Base image (rust toolchain)
├── Layer 2: Workdir /build
├── Layer 3: COPY Cargo.toml Cargo.lock (缓存优化)
├── Layer 4: COPY crates/ src/
├── Layer 5: RUN cargo build --release
└── Output: /build/target/release/stunserver

Stage 2: Runtime (alpine:3.19)
├── Layer 1: Base image (alpine minimal)
├── Layer 2: RUN apk add ca-certificates
├── Layer 3: WORKDIR /opt/stunserver
├── Layer 4: COPY --from=builder stunserver binary
├── Layer 5: COPY stunserver.conf.example
├── Layer 6: RUN mkdir log && chmod 755
├── Layer 7: EXPOSE 3478/udp 3478/tcp
├── Layer 8: ENTRYPOINT + CMD
└── Final Image Size: ~50-80MB
```

### Volume Mount Points

| Container Path              | Purpose                  | Host Path Example          | Required |
|-----------------------------|--------------------------|----------------------------|----------|
| /opt/stunserver/etc/stunserver.conf | 配置文件       | ./config/stunserver.conf   | No       |
| /opt/stunserver/log/        | 日志目录                 | ./logs/                    | Yes      |

### Environment Variables

| Variable            | Default | Description                          | Example              |
|---------------------|---------|--------------------------------------|----------------------|
| STUN_CONFIG_PATH    | /opt/stunserver/etc/stunserver.conf | 配置文件路径 | /custom/config.toml  |
| STUN_LOG_LEVEL      | info    | 日志级别（覆盖配置文件）             | debug                |
| STUN_PORT           | 3478    | 监听端口（覆盖配置文件）             | 3479                 |

### Container Lifecycle

```
[Image Built] --docker run--> [Container Created]
[Container Created] --start--> [Container Running]
[Container Running] --stop--> [Container Stopped]
[Container Stopped] --rm--> [Removed]
[Container Running] --crash--> [Container Exited] --restart policy--> [Container Running]
```

---

## Entity 5: systemd Service

### Service Configuration

```ini
[Unit]
Description=STUN Server - Session Traversal Utilities for NAT
Documentation=https://github.com/zhouhailin/stunserver
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=stunserver
Group=stunserver
ExecStart=/opt/stunserver/bin/stunserver --config /opt/stunserver/etc/stunserver.conf
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=stunserver

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/stunserver/log
PrivateTmp=true

# Resource limits
LimitNOFILE=65536

# Optional: Watchdog for automatic restart on hang
# WatchdogSec=30

[Install]
WantedBy=multi-user.target
```

### Service State Machine

```
[Loaded, Disabled] --enable--> [Loaded, Enabled]
[Loaded, Enabled] --start--> [Active, Running]
[Active, Running] --stop--> [Active, Stopped]
[Active, Running] --restart--> [Active, Running]
[Active, Running] --failure--> [Active, Failed] --RestartSec--> [Active, Running]
[Active, *] --disable--> [Loaded, Disabled]
[Loaded, *] --uninstall--> [Not Found]
```

### Dependencies

| Dependency        | Type    | Required | Notes                              |
|-------------------|---------|----------|------------------------------------|
| network.target    | After   | Yes      | 网络栈初始化后启动                 |
| network-online.target | Wants | No     | 等待网络完全就绪（可选）           |
| stunserver user   | User    | Yes      | 安装时创建的系统用户               |
| /opt/stunserver/log/ | Directory | Yes | 日志目录，需要写权限               |

---

## Relationships

```
Config File (stunserver.conf)
    ├── defines → ServerConfig, LoggingConfig, NetworkConfig, SecurityConfig
    ├── used by → STUN Server Binary
    └── mounted in → Docker Container (/opt/stunserver/etc/)

Directory Structure (/opt/stunserver/)
    ├── contains → bin/, etc/, log/
    ├── managed by → install/uninstall Makefile targets
    └── mounted as → Docker Volume

Log Files (stunserver.log.*)
    ├── written by → STUN Server Binary (RotatingLogger)
    ├── stored in → /opt/stunserver/log/
    ├── rotated by → Size/Time triggers
    └── persisted in → Docker Volume (/opt/stunserver/log/)

Docker Image
    ├── built from → Dockerfile (multi-stage)
    ├── contains → Binary, Config, Dependencies
    ├── exposes → Port 3478 (UDP/TCP)
    └── runs as → Docker Container

systemd Service
    ├── configured by → stunserver.service
    ├── manages → STUN Server Binary
    ├── runs as → stunserver user
    └── logs to → journald + /opt/stunserver/log/
```

---

## Validation Checklist

- [x] Configuration schema defined with all fields and defaults
- [x] Validation rules specified for each field
- [x] Rust data structures designed with serde support
- [x] Directory layout follows Linux standards
- [x] File permissions defined for security
- [x] Log rotation algorithm specified
- [x] Docker image layers optimized for cache
- [x] Volume mount points identified
- [x] Environment variables documented
- [x] systemd service configuration updated
- [x] Entity relationships mapped
- [x] Lifecycle states defined for all entities
