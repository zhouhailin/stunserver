# STUN 服务器

基于 Rust 实现的高性能 STUN（NAT 会话遍历实用工具）服务器，完全符合 RFC 5389 标准。

[English](README.md)

## 特性

- 🚀 **高性能**：基于 Rust 和 Tokio 异步 I/O 构建
- 🔒 **RFC 5389 兼容**：完整的 STUN 协议实现
- 📦 **易于部署**：标准化打包，集成 systemd 服务管理
- 🐳 **Docker 支持**：多阶段构建，镜像体积最小化
- 🔄 **CI/CD 就绪**：GitHub Actions 自动化构建和发布
- 📝 **完善的日志**：内置日志轮转和管理功能

## 快速开始

### 前置要求

- Rust 1.83+（从源码构建）
- Linux 系统，支持 systemd（部署）
- Docker 20.10+（容器化部署）

### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/zhouhailin/stunserver.git
cd stunserver

# 构建发布版本
make build

# 运行测试
make test

# 创建分发包
make package
```

### 安装（Linux + systemd）

```bash
# 先构建
make build

# 安装（需要 root 权限）
cd deploy
sudo make install

# 启动服务
sudo make start

# 查看状态
sudo make status
```

### Docker 部署

```bash
# 构建 Docker 镜像
docker build -t stunserver:latest -f docker/Dockerfile .

# 运行容器
docker run -d \
  --name stunserver \
  -p 3478:3478/udp \
  -p 3478:3478/tcp \
  -v $(pwd)/logs:/opt/stunserver/log \
  stunserver:latest
```

## 配置说明

服务器使用 TOML 格式的配置文件，位于 `/opt/stunserver/etc/stunserver.conf`。

### 配置示例

```toml
[server]
listen_address = "0.0.0.0"
port = 3478
worker_threads = 0  # 0 = 自动检测 CPU 核心数
max_connections = 10000

[logging]
level = "info"
log_dir = "/opt/stunserver/log"
file_prefix = "stunserver"
max_file_size_mb = 100
retention_days = 30
enable_console = true

[network]
udp_recv_buffer_size = 1048576
udp_send_buffer_size = 1048576
tcp_connection_timeout_sec = 30
tcp_idle_timeout_sec = 300
```

### 环境变量（Docker）

在 Docker 中运行时，可以通过环境变量覆盖配置：

- `STUN_CONFIG_PATH`：配置文件路径（默认：`/opt/stunserver/etc/stunserver.conf`）
- `STUN_LOG_LEVEL`：日志级别（trace, debug, info, warn, error）
- `STUN_PORT`：监听端口（默认：3478）
- `STUN_LISTEN_ADDRESS`：监听地址（默认：0.0.0.0）

## 目录结构

安装后，服务器使用以下目录结构：

```
/opt/stunserver/
├── bin/
│   └── stunserver              # 可执行文件
├── etc/
│   └── stunserver.conf         # 配置文件
└── log/
    ├── stunserver.log          # 当前日志文件
    └── stunserver.log.1        # 轮转后的日志文件
```

## 管理命令

### 开发命令（根目录 Makefile）

```bash
make build      # 构建发布版本
make test       # 运行所有测试
make package    # 创建分发包
make clean      # 清理构建产物
make fmt        # 格式化代码
make lint       # 运行 clippy 检查
make help       # 显示帮助
```

### 部署命令（deploy/Makefile）

```bash
sudo make install      # 安装并启用服务
sudo make uninstall    # 卸载服务和文件
sudo make start        # 启动服务
sudo make stop         # 停止服务
sudo make restart      # 重启服务
sudo make status       # 查看服务状态
sudo make help         # 显示帮助
```

## Docker 镜像

### 自动化构建

Docker 镜像通过 GitHub Actions 自动构建并发布到 GitHub Container Registry (ghcr.io)：

- **最新版本**：`ghcr.io/zhouhailin/stunserver:latest`（来自 main 分支）
- **版本标签**：`ghcr.io/zhouhailin/stunserver:v0.1.1`（来自 release）

### 拉取和运行

```bash
# 拉取最新镜像
docker pull ghcr.io/zhouhailin/stunserver:latest

# 使用自定义配置运行
docker run -d \
  --name stunserver \
  -p 3478:3478/udp \
  -p 3478:3478/tcp \
  -v $(pwd)/config/stunserver.conf:/opt/stunserver/etc/stunserver.conf:ro \
  -v $(pwd)/logs:/opt/stunserver/log \
  ghcr.io/zhouhailin/stunserver:latest
```

### Docker Compose

```yaml
version: '3.8'

services:
  stunserver:
    image: ghcr.io/zhouhailin/stunserver:latest
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
```

## 日志轮转

服务器内置日志轮转功能，具有以下特性：

- **基于大小轮转**：日志文件达到 100MB 时自动轮转（可配置）
- **基于时间轮转**：每天午夜自动轮转
- **保留策略**：保留最近 30 天的日志（可配置）
- **自动清理**：自动删除过期日志

日志文件命名规则：
- `stunserver.log` - 当前活跃日志
- `stunserver.log.1` - 最近轮转的日志
- `stunserver.log.2` - 更早的轮转日志
- ...

## 故障排除

### 服务无法启动

```bash
# 查看服务状态
sudo systemctl status stunserver

# 查看日志
sudo journalctl -u stunserver -n 50

# 检查配置文件
cat /opt/stunserver/etc/stunserver.conf
```

### 端口被占用

```bash
# 检查 3478 端口占用情况
sudo netstat -tulnp | grep 3478

# 修改配置文件中的端口
sudo vim /opt/stunserver/etc/stunserver.conf
# 修改：port = 3479

# 重启服务
sudo make restart
```

### 权限被拒绝

```bash
# 确保正确的文件所有权
sudo chown -R stunserver:stunserver /opt/stunserver

# 检查文件权限
ls -la /opt/stunserver/
```

### Docker 容器问题

```bash
# 查看容器日志
docker logs stunserver

# 检查容器状态
docker ps -a | grep stunserver

# 重启容器
docker restart stunserver
```

## 开发指南

### 项目结构

```
stunserver/
├── src/
│   ├── server/         # STUN 服务器实现
│   ├── client/         # STUN 客户端实现
│   └── bench/          # 性能测试工具
├── crates/
│   └── stun-protocol/  # 协议实现
├── deploy/             # 部署脚本
├── docker/             # Docker 文件
├── systemd/            # Systemd 服务文件
└── .github/workflows/  # CI/CD 工作流
```

### 运行测试

```bash
# 运行所有测试
cargo test

# 显示测试输出
cargo test -- --nocapture

# 运行特定测试
cargo test test_name
```

### 代码质量

```bash
# 格式化代码
cargo fmt --all

# 运行代码检查
cargo clippy -- -D warnings

# 检查格式
cargo fmt -- --check
```

## 贡献指南

欢迎贡献代码！请遵循以下步骤：

1. Fork 本仓库
2. 创建功能分支
3. 为新功能编写测试
4. 确保所有测试通过
5. 运行 `cargo fmt` 和 `cargo clippy`
6. 提交 Pull Request

## 许可证

本项目基于 MIT 许可证授权 - 详情请参见 [LICENSE](LICENSE) 文件。

## 相关链接

- **仓库地址**：https://github.com/zhouhailin/stunserver
- **问题反馈**：https://github.com/zhouhailin/stunserver/issues
- **RFC 5389**：https://tools.ietf.org/html/rfc5389
