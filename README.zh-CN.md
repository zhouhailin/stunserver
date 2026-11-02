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
- 📋 **RFC 一致性**：针对 RFC 5389/5769/5780 的可追溯审计矩阵，详见 [docs/rfc-conformance.md](docs/rfc-conformance.md)

## 协议范围

本服务器通过 UDP 实现 STUN Binding（RFC 5389）。它是单地址服务器：不提供 NAT 行为
发现能力，任何响应都不会包含 OTHER-ADDRESS 或 RESPONSE-ORIGIN。TCP、TLS 以及 RFC 3489
旧版兼容均不在支持范围内。

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

# 安装（需要 root 权限；同时安装 /etc/sysctl.d/99-stunserver.conf）
cd deploy
sudo make install

# 启动服务
sudo make start

# 查看状态
sudo make status
```

`make install` 还会应用 `systemd/99-stunserver.conf` 中的内核 UDP 缓冲上限；具体参数、
校验命令与容器场景见 [系统优化](#系统优化)。

### Docker 部署

```bash
# 构建 Docker 镜像
docker build -t stunserver:latest -f docker/Dockerfile .

# 运行容器（host 网络模式，无需端口映射）
docker run -d \
  --name stunserver \
  --network host \
  -v $(pwd)/logs:/opt/stunserver/log \
  stunserver:latest
```

默认使用 host 网络模式，容器直接绑定宿主机的 UDP/TCP 3478，避免 Docker NAT。Linux 上使用 `--network host` 后 `-p` 不再需要。如果必须使用 bridge 网络，请改为发布端口；客户端地址探测仍然正常，但服务端无法通过 NAT 得知自己的公网地址（见 [Docker 网络与 NAT](#docker-网络与-nat)）。

## 配置说明

服务器使用 TOML 格式的配置文件，位于 `/opt/stunserver/etc/stunserver.conf`。

### 配置示例

```toml
[server]
listen_address = "0.0.0.0"
port = 3478
worker_threads = 0  # 0 = 自动检测 CPU 核心数
max_connections = 10000  # 最大并发处理请求数；超出时丢弃数据报

[logging]
level = "info"
log_dir = "/opt/stunserver/log"
file_prefix = "stunserver"
max_file_size_mb = 100
retention_days = 30
enable_console = true
enable_file = false           # 文件日志默认关闭，需要时显式开启
compress_rotated_logs = true  # 轮转文件 gzip 压缩为 *.gz

[network]
udp_recv_buffer_size = 1048576
udp_send_buffer_size = 1048576
tcp_connection_timeout_sec = 30
tcp_idle_timeout_sec = 300
```

`udp_recv_buffer_size` 与 `udp_send_buffer_size` 会在创建 socket 时真正以 `SO_RCVBUF`/`SO_SNDBUF`
应用。内核会把它们静默截断到 `net.core.rmem_max` 与 `net.core.wmem_max`；推荐的上限、
持久化方式与 systemd 资源设置见 [系统优化](#系统优化)。

默认启用控制台日志。设置 `enable_file = true` 后才会额外写入 `log_dir` 下的轮转日志文件；关闭时不会创建日志目录。

### 环境变量

服务端启动时会读取以下环境变量，Docker 与直接部署行为一致：

- `STUN_CONFIG_PATH`：配置文件路径（默认：`config/stunserver.conf`）
- `STUN_LOG_LEVEL`：日志级别（trace, debug, info, warn, error）
- `STUN_PORT`：监听端口（默认：3478）
- `STUN_LISTEN_ADDRESS`：监听地址（默认：0.0.0.0）

优先级为 CLI 参数 > 环境变量 > 配置文件 > 内置默认值。空值视为未设置，显式指定的配置文件必须存在。

### 认证配置

服务端通过 `[auth]` 支持 `disabled`（默认）、`short-term`、`long-term` 三种模式。启用认证后不会回退到未认证的成功响应。

```toml
[auth]
mode = "short-term"   # "disabled"、"short-term" 或 "long-term"

[[auth.users]]
username = "alice"
password = "change-me"

# 仅 long-term：realm 必填；nonce_lifetime_sec 默认 600
# mode = "long-term"
# realm = "example.org"
# nonce_lifetime_sec = 600
```

`long-term` 模式下，客户端先收到携带 REALM 与 NONCE 的未签名 401 挑战，再以签名请求重试。NONCE 过期时返回 438 并附带新 NONCE，客户端会采用它，最多进行两次带签名的重试；签名请求之后再收到 401 则直接终止（RFC 5389 §10.2.3 禁止在未更换凭据的情况下重试）。`nonce_lifetime_sec` 取值范围为 1 到 86400。

USERNAME、REALM 和密码均按 SASLprep 处理（RFC 5389 §15.3、§15.4、§15.7）；USERNAME 最长 512 字节，REALM/NONCE 少于 128 个字符。自带 UDP 客户端按 §7.2.1 重传请求：初始 RTO 为 500 ms，每次翻倍，最多发送 7 次；最后一次发送后再等待 16×RTO（8 秒）；`--timeout` 是整笔交换的硬上限。来源地址不符、事务 ID 不一致或校验失败的响应会被静默丢弃，客户端继续等待有效响应（§7.3.3、§10.1.3）。400 错误允许不带 MESSAGE-INTEGRITY，因为服务器可能无法为其计算有效摘要（§15.6）。`stunclient` 每次调用只执行一次探测交换，因此不涉及跨请求缓存 REALM/NONCE（§10.2.1.2）或按服务器缓存 RTO（§7.2.1）。

配置文件以明文保存密码，请仅允许服务账号读取：

```bash
chmod 600 /opt/stunserver/etc/stunserver.conf
```

不要提交生产凭据。自带客户端优先使用 `STUNCLIENT_PASSWORD` 环境变量而非 `--password`，因为命令行参数会出现在进程列表中：

```bash
STUNCLIENT_PASSWORD='change-me' stunclient --auth-mode long-term --username alice stun.example.org:3478
```

`--realm` 可用于诊断时覆盖 long-term 挑战下发的 REALM（挑战中的 NONCE 仍会使用）。真实服务端始终按自己配置的 REALM 派生密钥，因此覆盖值与服务端不一致时通常返回 401。

MESSAGE-INTEGRITY 只保证完整性，UDP 流量不会被加密。

## 系统优化

在高包速率下，除了 `[network]` 配置外还需要内核侧的余量。推荐值随
`systemd/99-stunserver.conf` 提供，并由 `deploy/Makefile` 安装到
`/etc/sysctl.d/99-stunserver.conf`：

| 参数 | 值 | 作用 |
|------|----|------|
| `net.core.rmem_max` | `16777216` | `SO_RCVBUF` 的上限；不提高时，配置的 `udp_recv_buffer_size` 会被静默截断（常见为 212992 字节，在几万 QPS 下只有几毫秒的缓冲）。 |
| `net.core.wmem_max` | `16777216` | `SO_SNDBUF` 的上限；同样限制 `udp_send_buffer_size`。 |
| `net.core.netdev_max_backlog` | `16384` | 网卡驱动到协议栈之间的队列；若 `/proc/net/softnet_stat` 的 dropped 列持续增长，应提高该值。 |

应用并确认：

```bash
sudo cp systemd/99-stunserver.conf /etc/sysctl.d/99-stunserver.conf
sudo sysctl --system
sysctl net.core.rmem_max net.core.wmem_max net.core.netdev_max_backlog
```

内核在内部通常按请求值的 2 倍记账，因此 4 MiB 的请求实际受限于
`2 × net.core.rmem_max` / `2 × net.core.wmem_max`。在容器中运行服务时，这些上限来自宿主机
内核；请直接在宿主机上设置（除非容器使用独立的网络命名空间）。`stunbench` 为其自身的
UDP socket 请求 4 MiB 缓冲，同样受上述内核上限约束。

systemd 单元（`systemd/stunserver.service`）额外设置了 `LimitNOFILE=1048576`，日志写入
journal。内核参数是全局的，修改 sysctl drop-in 或 `[network]` 缓冲大小后需要重启服务，
以便用新参数重新创建监听 socket：

```bash
sudo systemctl restart stunserver
```

删除 `/etc/sysctl.d/99-stunserver.conf` 后，后续启动不再提高上限，但当前已生效的值会保留到
重启。

### 示例：50000 QPS 压测

应用上述参数并将 `udp_recv_buffer_size`/`udp_send_buffer_size` 设为 1048576 后，
`stunbench --qps 50000` 在回环测试中完成 3,000,000 次请求、0 失败，p99 = 0.92 ms。

## 目录结构

安装后，服务器使用以下目录结构：

```
/opt/stunserver/
├── bin/
│   └── stunserver              # 可执行文件
├── etc/
│   └── stunserver.conf         # 配置文件
└── log/
    ├── stunserver.log                    # 当前日志文件
    └── stunserver.log.2026-10-31.1       # 轮转后的日志文件
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

# 使用自定义配置运行（host 网络模式）
docker run -d \
  --name stunserver \
  --network host \
  -v $(pwd)/config/stunserver.conf:/opt/stunserver/etc/stunserver.conf:ro \
  -v $(pwd)/logs:/opt/stunserver/log \
  ghcr.io/zhouhailin/stunserver:latest
```

### Docker Compose

```yaml
services:
  stunserver:
    image: ghcr.io/zhouhailin/stunserver:latest
    container_name: stunserver
    network_mode: host
    volumes:
      - ./config/stunserver.conf:/opt/stunserver/etc/stunserver.conf:ro
      - ./logs:/opt/stunserver/log
    environment:
      - STUN_LOG_LEVEL=info
    restart: unless-stopped
```

#### 无卷最小配置

镜像内已包含 `/opt/stunserver/etc/stunserver.conf`，且默认启用控制台日志、关闭文件日志，因此以下配置不需要任何 volume：

```yaml
services:
  stunserver:
    image: ghcr.io/zhouhailin/stunserver:latest
    container_name: stunserver
    network_mode: host
    environment:
      - STUN_LOG_LEVEL=info
    restart: unless-stopped
```

等价的 `docker run` 命令：

```bash
docker run -d \
  --name stunserver \
  --network host \
  ghcr.io/zhouhailin/stunserver:latest
```

### Docker 网络与 NAT

`docker run` 与 Compose 默认都使用 host 网络模式。容器共享宿主机的网络命名空间，服务直接绑定 `0.0.0.0:3478`，不经过容器 NAT，也不需要 `-p`。同一端口只能被一个进程占用，且容器会暴露宿主机的所有网络接口，请按需配置防火墙。macOS/Windows 上的 Docker Desktop 对 host 网络支持有限，请确认版本或继续使用 bridge 模式。

bridge 模式（`-p 3478:3478/udp -p 3478:3478/tcp`）同样可以正常提供 STUN 服务：服务端返回的是它看到的客户端源地址，也就是客户端经过 NAT 后的公网地址。但服务端自身只能看到容器内网地址和转发后的端口，无法从 STUN 流量中推断宿主机的公网 IP 或外部端口。RFC 5389 Binding 并不需要服务端知道自己的公网地址，本服务也未实现 RFC 5780 的 NAT 行为发现，因此不会返回 RESPONSE-ORIGIN 或 OTHER-ADDRESS。如果后续需要对外通告公网地址，必须手动配置或通过外部服务探测，目前没有该配置项。

## 日志轮转

服务器内置日志轮转功能，具有以下特性：

- **基于大小轮转**：日志文件达到 100MB 时自动轮转（可配置）
- **基于时间轮转**：每天午夜自动轮转
- **保留策略**：保留最近 30 天的日志（可配置）
- **自动清理**：自动删除过期日志

日志文件命名规则：
- `stunserver.log` - 当前活跃日志
- `stunserver.log.YYYY-MM-DD.1` - 当天第 1 个轮转日志
- `stunserver.log.YYYY-MM-DD.2` - 当天第 2 个轮转日志
- ...

序号每天从 1 开始，不补零，且不会覆盖已有文件。

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
├── deploy/             # 部署脚本（systemd 安装器、配置示例）
├── docker/             # Docker 文件
├── systemd/            # Systemd 服务单元与内核调优 drop-in
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
