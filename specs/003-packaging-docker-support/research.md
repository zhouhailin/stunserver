# Phase 0: Research & Unknowns Resolution

**Date**: 2026-10-29  
**Status**: ✅ Complete

## Research Tasks

### 1. 配置文件格式选择

**Decision**: 使用 TOML 格式（stunserver.conf）

**Rationale**:
- TOML 是 Rust 生态系统的标准配置格式（Cargo.toml 本身使用 TOML）
- 语法简洁易读，适合系统配置文件
- 有成熟的解析库（toml crate）
- 支持注释，便于用户理解配置项
- 类型安全，支持嵌套结构

**Alternatives Considered**:
- **YAML**: 已被 serde_yaml 使用，但语法对非技术用户不够友好，缩进敏感容易出错
- **JSON**: 不支持注释，语法冗长，不适合手动编辑的配置文件
- **INI**: 过于简单，不支持嵌套结构，Rust 生态支持较弱

**Implementation Notes**:
- 添加 `toml = "0.8"` 依赖到 Cargo.toml
- 保持 serde_yaml 用于其他可能的用途（如测试数据）
- 配置文件路径：/opt/stunserver/etc/stunserver.conf
- 提供示例配置：deploy/stunserver.conf.example

---

### 2. 日志轮转实现策略

**Decision**: 应用内置日志轮转，基于文件大小和时间策略

**Rationale**:
- 无需外部依赖（如 logrotate），简化部署
- 可以在应用层面控制轮转时机，避免日志丢失
- 适合容器化部署（容器内通常没有 logrotate）
- 可以实现更精细的轮转策略（如按大小+时间双重条件）

**Alternatives Considered**:
- **logrotate**: 需要系统安装和配置，容器中不可用，增加部署复杂度
- **systemd-journald**: 依赖 systemd 日志系统，不够通用，日志格式不受应用控制
- **外部日志服务**: 过度设计，增加网络依赖和延迟

**Implementation Strategy**:
```rust
// 伪代码示例
pub struct RotatingLogger {
    current_file: File,
    current_size: u64,
    max_size: u64,           // 100MB
    retention_days: u32,     // 30天
    log_dir: PathBuf,
}

impl RotatingLogger {
    pub fn write(&mut self, msg: &str) -> Result<()> {
        // 1. 检查是否需要轮转（大小或时间）
        // 2. 如需轮转：重命名当前文件，创建新文件
        // 3. 清理过期日志
        // 4. 写入消息
    }
}
```

**Key Design Decisions**:
- 轮转触发条件：文件大小 >= 100MB 或 日期变更（午夜）
- 文件命名：stunserver.log, stunserver.log.1, stunserver.log.2, ...
- 清理策略：保留最近 30 天的日志文件
- 异步写入：使用 tokio::fs 避免阻塞主线程
- 原子轮转：先写入新文件，再重命名旧文件，避免日志丢失

**Dependencies**:
- 使用现有的 `log` 和 `env_logger` crate
- 实现自定义 `Log` trait 以支持轮转
- 或使用 `tracing` + `tracing-appender` crate（更现代的选择）

---

### 3. Docker 镜像构建策略

**Decision**: 多阶段构建，基于 Alpine Linux

**Rationale**:
- 多阶段构建显著减小镜像体积（最终镜像不包含编译工具链）
- Alpine Linux 基础镜像小（~5MB），安全性高
- 符合成功标准 SC-008（镜像大小 < 200MB）
- 构建缓存优化：先复制 Cargo.toml 和 Cargo.lock，再复制源码

**Alternatives Considered**:
- **单阶段构建**: 镜像体积大（包含 rustc、cargo 等），不安全
- **Debian/Ubuntu 基础镜像**: 体积较大（~100MB），攻击面大
- **distroless 镜像**: 不包含 shell，调试困难，不适合需要配置文件的应用
- **scratch 镜像**: 过于精简，缺少 CA 证书等基本工具

**Dockerfile Structure**:
```dockerfile
# Stage 1: Builder
FROM rust:1.75-alpine AS builder
WORKDIR /build
# 复制依赖文件（利用缓存）
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY src/ ./src/
# 构建 release 版本
RUN cargo build --release

# Stage 2: Runtime
FROM alpine:3.19
RUN apk add --no-cache ca-certificates
WORKDIR /opt/stunserver
# 从 builder 复制二进制
COPY --from=builder /build/target/release/stunserver ./bin/
# 复制默认配置
COPY deploy/stunserver.conf.example ./etc/stunserver.conf
# 创建日志目录
RUN mkdir -p ./log && chmod 755 ./log
# 暴露端口
EXPOSE 3478/udp 3478/tcp
# 入口点
ENTRYPOINT ["./bin/stunserver"]
CMD ["--config", "./etc/stunserver.conf"]
```

**Expected Image Size**: ~50-80MB (Alpine base + Rust binary + dependencies)

**Build Optimization**:
- 使用 .dockerignore 排除 target/、.git/、docs/ 等
- 利用 Docker 层缓存：先复制 Cargo.toml，运行 cargo build --release --locked
- 使用 cargo-chef 进一步优化（可选，如果构建时间成为瓶颈）

---

### 4. systemd 服务配置最佳实践

**Decision**: 更新现有 systemd/stunserver.service，遵循 systemd 最佳实践

**Rationale**:
- 现有服务文件结构良好，只需更新路径和 URL
- 遵循 Linux 标准路径约定
- 安全加固选项已启用（NoNewPrivileges、ProtectSystem 等）

**Required Changes**:
```diff
- Documentation=https://github.com/stunserver/stunserver
+ Documentation=https://github.com/zhouhailin/stunserver

- ExecStart=/usr/local/bin/stunserver --config /etc/stunserver/config.yaml
+ ExecStart=/opt/stunserver/bin/stunserver --config /opt/stunserver/etc/stunserver.conf

- ReadWritePaths=/var/log/stunserver
+ ReadWritePaths=/opt/stunserver/log
```

**Security Hardening** (already present, verify):
- NoNewPrivileges=true ✅
- ProtectSystem=strict ✅
- ProtectHome=true ✅
- PrivateTmp=true ✅
- LimitNOFILE=65536 ✅

**Additional Recommendations**:
- 添加 WatchdogSec=30 用于自动重启卡死的服务
- 添加 MemoryMax=512M 限制内存使用（可选）
- 考虑添加 SystemCallFilter 限制系统调用（高级安全加固）

---

### 5. Makefile 设计模式

**Decision**: 分离两个 Makefile，根目录用于开发，deploy/ 用于部署

**Rationale**:
- 职责分离：开发命令和部署命令不混合
- 权限分离：deploy/Makefile 需要 root，根目录 Makefile 不需要
- 清晰的用户体验：开发者使用根目录 Makefile，运维使用 deploy/Makefile
- 易于维护：每个文件专注于特定场景

**Root Makefile Structure** (开发命令):
```makefile
.PHONY: build test package clean fmt lint help

# 默认目标
all: build

# 构建
build:
	cargo build --release

# 测试
test:
	cargo test
	cargo test -- --ignored

# 打包
package: build
	@echo "Creating distribution package..."
	@mkdir -p dist/stunserver-{bin,etc,log}
	@cp target/release/stunserver dist/stunserver-bin/
	@cp deploy/stunserver.conf.example dist/stunserver-etc/stunserver.conf
	@cp systemd/stunserver.service dist/
	@tar -czf dist/stunserver-$(version).tar.gz -C dist stunserver-*
	@echo "Package created: dist/stunserver-$(version).tar.gz"

# 清理
clean:
	cargo clean
	rm -rf dist/

# 格式化
fmt:
	cargo fmt --all

# Lint
lint:
	cargo clippy -- -D warnings

# 帮助
help:
	@echo "Available commands:"
	@echo "  make build    - Build release binary"
	@echo "  make test     - Run all tests"
	@echo "  make package  - Create distribution package"
	@echo "  make clean    - Clean build artifacts"
	@echo "  make fmt      - Format code"
	@echo "  make lint     - Run clippy"
```

**deploy/Makefile Structure** (部署命令):
```makefile
.PHONY: install uninstall start stop restart status help

INSTALL_DIR = /opt/stunserver
SERVICE_NAME = stunserver

# 安装
install:
	@echo "Installing STUN server to $(INSTALL_DIR)..."
	@sudo mkdir -p $(INSTALL_DIR)/{bin,etc,log}
	@sudo cp ../target/release/stunserver $(INSTALL_DIR)/bin/
	@sudo cp stunserver.conf.example $(INSTALL_DIR)/etc/stunserver.conf
	@sudo cp ../systemd/stunserver.service /etc/systemd/system/
	@sudo systemctl daemon-reload
	@sudo systemctl enable $(SERVICE_NAME)
	@sudo useradd -r -s /bin/false $(SERVICE_NAME) 2>/dev/null || true
	@sudo chown -R $(SERVICE_NAME):$(SERVICE_NAME) $(INSTALL_DIR)
	@echo "Installation complete. Service enabled."

# 卸载
uninstall:
	@echo "Uninstalling STUN server..."
	@sudo systemctl stop $(SERVICE_NAME) 2>/dev/null || true
	@sudo systemctl disable $(SERVICE_NAME) 2>/dev/null || true
	@sudo rm -f /etc/systemd/system/$(SERVICE_NAME).service
	@sudo systemctl daemon-reload
	@sudo rm -rf $(INSTALL_DIR)
	@sudo userdel $(SERVICE_NAME) 2>/dev/null || true
	@echo "Uninstallation complete."

# 启动
start:
	@sudo systemctl start $(SERVICE_NAME)
	@echo "Service started."

# 停止
stop:
	@sudo systemctl stop $(SERVICE_NAME)
	@echo "Service stopped."

# 重启
restart:
	@sudo systemctl restart $(SERVICE_NAME)
	@echo "Service restarted."

# 状态
status:
	@sudo systemctl status $(SERVICE_NAME)

# 帮助
help:
	@echo "Available commands (requires root):"
	@echo "  make install   - Install and enable service"
	@echo "  make uninstall - Stop, disable, and remove service"
	@echo "  make start     - Start service"
	@echo "  make stop      - Stop service"
	@echo "  make restart   - Restart service"
	@echo "  make status    - Show service status"
```

**Edge Case Handling**:
- systemd 检测：install 命令开始时检查 `command -v systemctl`，不存在则报错
- 权限检查：检查 `id -u` 是否为 0，非 root 则提示使用 sudo
- 已存在文件：install 前检查 $(INSTALL_DIR) 是否存在，存在则备份或提示确认

---

## Research Summary

| Topic                    | Decision                          | Risk Level | Mitigation                              |
|--------------------------|-----------------------------------|------------|-----------------------------------------|
| Configuration format     | TOML (stunserver.conf)            | Low        | 成熟库支持，Rust 生态标准               |
| Log rotation             | Application built-in              | Medium     | 充分测试轮转逻辑，使用异步 I/O          |
| Docker image             | Multi-stage, Alpine base          | Low        | 标准模式，体积可预测                    |
| systemd service          | Update existing, follow best practices | Low   | 已有良好基础，仅需路径更新              |
| Makefile structure       | Separate dev and deploy Makefiles | Low        | 职责清晰，易于维护                      |

**All NEEDS CLARIFICATION items resolved**: ✅

**Ready for Phase 1**: ✅
