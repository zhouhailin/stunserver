# Contract: Docker Interface

**Version**: 1.0  
**Date**: 2026-10-29  
**Status**: Defined

## Overview

本文档定义 STUN 服务器 Docker 镜像的构建、运行、配置接口契约。

---

## Contract 1: Docker Image Build

### Image Name

**Format**: `stunserver:{version}` 或 `stunserver:latest`

**Examples**:
- `stunserver:0.1.1` — 特定版本
- `stunserver:latest` — 最新版本
- `stunserver:0.1` — 次要版本系列
- `stunserver:0` — 主要版本系列

### Build Command

```bash
docker build -t stunserver:{version} -f docker/Dockerfile .
```

### Build Context

**Required Files**:
- `Cargo.toml` — workspace 配置
- `Cargo.lock` — 依赖锁定
- `src/` — 源码目录
- `crates/` — 子 crate 目录
- `deploy/stunserver.conf.example` — 示例配置
- `docker/Dockerfile` — Docker 构建文件
- `docker/docker-entrypoint.sh` — 容器入口脚本

**Ignored Files** (.dockerignore):
```
target/
.git/
.gitignore
.github/
docs/
specs/
*.md
!README.md
```

### Build Stages

**Stage 1: Builder**
- Base image: `rust:1.75-alpine`
- Purpose: 编译 Rust 源码为 release 二进制
- Output: `/build/target/release/stunserver`
- Cache optimization: 先复制 Cargo.toml/Cargo.lock，再复制源码

**Stage 2: Runtime**
- Base image: `alpine:3.19`
- Purpose: 最小化运行时环境
- Layers:
  1. 安装 ca-certificates（用于 HTTPS）
  2. 设置工作目录 /opt/stunserver
  3. 复制二进制文件
  4. 复制默认配置
  5. 创建日志目录
  6. 暴露端口
  7. 设置入口点

### Expected Image Size

**Target**: < 200MB (Success Criteria SC-008)  
**Estimated**: 50-80MB

**Breakdown**:
- Alpine base: ~5MB
- ca-certificates: ~1MB
- Rust binary (stripped): ~10-15MB
- Dependencies (libc, etc.): ~5-10MB
- Config files: <1MB
- **Total**: ~22-32MB

### Build Time

**Target**: < 10 minutes (Success Criteria SC-003)  
**Estimated**: 3-5 minutes (with cache)

**Optimization Strategies**:
1. 层缓存：先复制 Cargo.toml，运行 `cargo build --release --locked`
2. 多阶段构建：最终镜像不包含编译工具链
3. .dockerignore：排除不必要的文件
4. 并行编译：Rust 默认支持

---

## Contract 2: Docker Image Run

### Basic Run Command

```bash
docker run -d \
  --name stunserver \
  -p 3478:3478/udp \
  -p 3478:3478/tcp \
  -v ./logs:/opt/stunserver/log \
  stunserver:latest
```

### Port Mapping

| Container Port | Protocol | Host Port | Purpose              | Required |
|----------------|----------|-----------|----------------------|----------|
| 3478           | UDP      | 3478      | STUN over UDP        | Yes      |
| 3478           | TCP      | 3478      | STUN over TCP        | Yes      |

**Note**: 端口可以在配置文件中修改，但 Docker 镜像默认暴露 3478。

### Volume Mounts

| Container Path              | Purpose      | Host Path Example        | Required | Read/Write |
|-----------------------------|--------------|--------------------------|----------|------------|
| /opt/stunserver/etc/stunserver.conf | 配置文件 | ./config/stunserver.conf | No       | Read       |
| /opt/stunserver/log/        | 日志目录     | ./logs/                  | Yes      | Write      |

**Default Behavior**:
- 如果不挂载配置文件，使用镜像内置的默认配置
- 日志目录必须挂载，否则容器删除后日志丢失

### Environment Variables

| Variable            | Default                              | Description                          | Required |
|---------------------|--------------------------------------|--------------------------------------|----------|
| STUN_CONFIG_PATH    | /opt/stunserver/etc/stunserver.conf  | 配置文件路径                         | No       |
| STUN_LOG_LEVEL      | (from config)                        | 日志级别（覆盖配置文件）             | No       |
| STUN_PORT           | (from config)                        | 监听端口（覆盖配置文件）             | No       |
| STUN_LISTEN_ADDRESS | (from config)                        | 监听地址（覆盖配置文件）             | No       |

**Precedence**: 环境变量 > 配置文件 > 默认值

**Example**:
```bash
docker run -d \
  -e STUN_LOG_LEVEL=debug \
  -e STUN_PORT=3479 \
  -p 3479:3479/udp \
  -p 3479:3479/tcp \
  stunserver:latest
```

### Entrypoint and CMD

**Entrypoint**: `/opt/stunserver/bin/stunserver`  
**Default CMD**: `["--config", "/opt/stunserver/etc/stunserver.conf"]`

**Override Example**:
```bash
# 使用自定义配置
docker run -d \
  -v ./my-config.toml:/opt/stunserver/etc/stunserver.conf:ro \
  stunserver:latest

# 传递额外参数
docker run -d \
  stunserver:latest \
  --config /opt/stunserver/etc/stunserver.conf \
  --verbose
```

### Health Check

**Recommended**: 添加 HEALTHCHECK 指令到 Dockerfile

```dockerfile
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD nc -z localhost 3478 || exit 1
```

**Alternative**: 使用 STUN Binding Request 进行健康检查（需要 stun-client 工具）

### Resource Limits

**Recommended Docker Run Options**:
```bash
docker run -d \
  --name stunserver \
  --memory=512m \
  --cpus=2.0 \
  --restart=unless-stopped \
  stunserver:latest
```

| Resource | Limit    | Notes                          |
|----------|----------|--------------------------------|
| Memory   | 512MB    | 根据 max_connections 调整      |
| CPU      | 2.0      | 根据 worker_threads 调整       |
| Restart  | unless-stopped | 容器崩溃后自动重启       |

---

## Contract 3: Docker Compose Integration

### docker-compose.yml Example

```yaml
version: '3.8'

services:
  stunserver:
    image: stunserver:latest
    container_name: stunserver
    ports:
      - "3478:3478/udp"
      - "3478:3478/tcp"
    volumes:
      - ./config/stunserver.conf:/opt/stunserver/etc/stunserver.conf:ro
      - ./logs:/opt/stunserver/log
    environment:
      - STUN_LOG_LEVEL=info
    restart: unless-stopped
    mem_limit: 512m
    cpus: 2.0
    healthcheck:
      test: ["CMD", "nc", "-z", "localhost", "3478"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 5s
```

### Compose Commands

**Start**:
```bash
docker-compose up -d
```

**Stop**:
```bash
docker-compose down
```

**Logs**:
```bash
docker-compose logs -f stunserver
```

**Restart**:
```bash
docker-compose restart stunserver
```

---

## Contract 4: Container Lifecycle

### States

```
[Image Built]
    ↓ docker run
[Container Created]
    ↓ start
[Container Running] ←──── restart
    ↓ stop                ↑
[Container Stopped] ──────┘
    ↓ rm
[Container Removed]

[Container Running]
    ↓ crash/error
[Container Exited]
    ↓ restart policy
[Container Running] (auto-restart)
```

### Startup Sequence

1. 容器启动
2. 执行 docker-entrypoint.sh
3. 检查环境变量，生成或复制配置文件
4. 启动 stunserver 二进制
5. 服务监听 3478 端口
6. 健康检查开始

### Shutdown Sequence

1. 收到 SIGTERM 信号
2. stunserver 优雅关闭（等待现有连接完成）
3. 关闭监听套接字
4. 刷新日志缓冲区
5. 退出进程（退出码 0）
6. 容器停止

### Crash Recovery

**Restart Policies**:
- `no` — 不自动重启（默认）
- `on-failure[:max-retries]` — 失败时重启（可选最大重试次数）
- `always` — 总是重启
- `unless-stopped` — 除非手动停止，否则总是重启

**Recommended**: `unless-stopped` 或 `on-failure:5`

---

## Contract 5: Logging in Containers

### Log Output Destinations

| Destination          | Enabled By Default | Configuration                    |
|----------------------|--------------------|----------------------------------|
| Container stdout     | Yes                | logging.enable_console = true    |
| File (/opt/stunserver/log/) | Yes         | logging.log_dir                  |
| Docker logs driver   | Yes                | Docker daemon 配置               |

### Accessing Logs

**Container stdout**:
```bash
docker logs stunserver
docker logs -f stunserver  # 实时跟踪
docker logs --tail 100 stunserver  # 最后 100 行
```

**Log files** (if volume mounted):
```bash
tail -f ./logs/stunserver.log
```

### Log Rotation in Containers

**Challenge**: 容器内通常没有 logrotate

**Solution**: 应用内置日志轮转（已在 data-model.md 中定义）

**Behavior**:
- 应用自动检测日志文件大小
- 达到 100MB 时自动轮转
- 保留最近 30 天的日志
- 轮转操作对容器外部透明

**Volume Mount Recommendation**:
```bash
-v ./logs:/opt/stunserver/log
```
这样可以在宿主机访问轮转后的日志文件。

---

## Contract 6: Security Considerations

### User Context

**Default**: 容器以 root 用户运行（Docker 默认行为）

**Recommendation**: 使用非 root 用户运行

```dockerfile
# 在 Dockerfile 中添加
RUN adduser -D -u 1000 stunserver
USER stunserver
```

**Run with user**:
```bash
docker run -d --user 1000:1000 stunserver:latest
```

### Read-Only Filesystem

**Recommendation**: 使用只读文件系统提高安全性

```bash
docker run -d \
  --read-only \
  --tmpfs /tmp \
  -v ./logs:/opt/stunserver/log \
  stunserver:latest
```

**Note**: 日志目录必须是可写的（通过 volume mount）。

### Network Security

**Exposed Ports**:
- 3478/udp — STUN over UDP
- 3478/tcp — STUN over TCP

**Firewall Rules**:
```bash
# 仅允许特定 IP 访问
iptables -A INPUT -p udp --dport 3478 -s 192.168.1.0/24 -j ACCEPT
iptables -A INPUT -p udp --dport 3478 -j DROP
```

### Secrets Management

**Configuration Files**: 可能包含敏感信息（如 TLS 私钥路径）

**Recommendation**:
- 使用 Docker secrets 或环境变量传递敏感配置
- 不要将包含敏感信息的配置文件打包到镜像中
- 使用 volume mount 挂载敏感配置文件，并设置适当权限

```bash
docker run -d \
  -v ./secrets/tls-key.pem:/opt/stunserver/etc/tls-key.pem:ro \
  --tmpfs /opt/stunserver/etc:ro \
  stunserver:latest
```

---

## Contract 7: Image Distribution

### Registry

**Recommended Registries**:
- Docker Hub: `docker.io/stunserver/stunserver`
- GitHub Container Registry: `ghcr.io/zhouhailin/stunserver`
- Private Registry: `registry.example.com/stunserver`

### Tagging Strategy

| Tag Pattern        | Example              | Purpose                          |
|--------------------|----------------------|----------------------------------|
| {version}          | 0.1.1                | 特定版本，不可变                 |
| {major}.{minor}    | 0.1                  | 次要版本系列，自动更新 patch     |
| {major}            | 0                    | 主要版本系列，自动更新 minor     |
| latest             | latest               | 最新版本，不稳定                 |
| sha-{commit}       | sha-abc1234          | 特定提交，用于调试               |

### Push Command

```bash
# 登录 registry
docker login

# 打标签
docker tag stunserver:0.1.1 username/stunserver:0.1.1
docker tag stunserver:0.1.1 username/stunserver:latest

# 推送
docker push username/stunserver:0.1.1
docker push username/stunserver:latest
```

---

## Validation Checklist

- [x] Image build process defined
- [x] Multi-stage build implemented
- [x] Image size < 200MB
- [x] Port exposure documented
- [x] Volume mounts specified
- [x] Environment variables defined
- [x] Entrypoint and CMD configured
- [x] Health check recommended
- [x] Resource limits suggested
- [x] Docker Compose example provided
- [x] Container lifecycle documented
- [x] Logging strategy defined
- [x] Security considerations addressed
- [x] Image distribution strategy planned
