# Quickstart: Validation Guide

**Date**: 2026-10-29  
**Status**: Ready for Testing

## Overview

本指南提供端到端的验证场景，证明打包优化与 Docker 镜像支持功能正常工作。所有场景均可独立执行，无需完整实现即可验证单个功能点。

**Prerequisites**:
- Linux 系统（Ubuntu 20.04+, CentOS 7+, Debian 10+）
- systemd 已安装并运行
- root 权限（或 sudo 访问）
- Docker 引擎 20.10+（用于容器化测试）
- Rust 工具链 1.75+（用于构建测试）

---

## Scenario 1: 开发构建流程验证

**Goal**: 验证根目录 Makefile 的开发命令正常工作

**Time**: ~5 分钟

### Steps

1. **构建二进制文件**
   ```bash
   cd /opt/stunserver
   make build
   ```
   
   **Expected Output**:
   ```
   cargo build --release
      Compiling stun-server v0.1.1
       Finished release [optimized] target(s) in 2m 30s
   ```
   
   **Verify**:
   ```bash
   ls -lh target/release/stunserver
   file target/release/stunserver
   ```
   
   **Expected**: 二进制文件存在，显示 "ELF 64-bit LSB executable"

2. **运行测试**
   ```bash
   make test
   ```
   
   **Expected Output**:
   ```
   cargo test
      Compiling stun-server v0.1.1
       Finished test [unoptimized + debuginfo] target(s) in 1m 15s
      Running unittests
   test result: ok. 42 passed; 0 failed; 0 ignored
   ```
   
   **Verify**: 所有测试通过，退出码 0

3. **代码格式化检查**
   ```bash
   cargo fmt -- --check
   ```
   
   **Expected**: 退出码 0（无格式问题）或显示需要格式化的文件列表

4. **Clippy lint 检查**
   ```bash
   make lint
   ```
   
   **Expected Output**:
   ```
   cargo clippy -- -D warnings
       Checking stun-server v0.1.1
       Finished dev [unoptimized + debuginfo] target(s) in 0.50s
   ```
   
   **Verify**: 退出码 0，无警告

5. **创建分发包**
   ```bash
   make package VERSION=0.1.1
   ```
   
   **Expected Output**:
   ```
   Creating distribution package...
   Package created: dist/stunserver-0.1.1.tar.gz
   ```
   
   **Verify**:
   ```bash
   tar -tzf dist/stunserver-0.1.1.tar.gz
   ```
   
   **Expected**: 包含 stunserver-bin/, stunserver-etc/, stunserver-log/, stunserver.service

### Success Criteria

- ✅ `make build` 成功生成二进制文件
- ✅ `make test` 所有测试通过
- ✅ `make lint` 无警告
- ✅ `make package` 生成完整的分发包

---

## Scenario 2: 安装与部署流程验证

**Goal**: 验证 deploy/Makefile 的安装、启动、停止、卸载命令

**Time**: ~10 分钟

### Steps

1. **前置检查**
   ```bash
   # 确保已构建
   make build
   
   # 检查 systemd
   command -v systemctl && echo "systemd found" || echo "systemd not found"
   
   # 检查 root 权限
   id -u
   ```
   
   **Expected**: systemd 存在，当前用户 ID 为 0（root）或使用 sudo

2. **安装服务**
   ```bash
   cd deploy
   sudo make install
   ```
   
   **Expected Output**:
   ```
   Installing STUN server to /opt/stunserver...
   Installation complete. Service enabled.
   ```
   
   **Verify**:
   ```bash
   # 检查目录结构
   ls -la /opt/stunserver/
   ls -la /opt/stunserver/bin/
   ls -la /opt/stunserver/etc/
   ls -la /opt/stunserver/log/
   
   # 检查 systemd 服务
   systemctl is-enabled stunserver
   systemctl status stunserver
   ```
   
   **Expected**:
   - 目录结构：bin/, etc/, log/ 存在
   - 二进制文件：/opt/stunserver/bin/stunserver 存在且可执行
   - 配置文件：/opt/stunserver/etc/stunserver.conf 存在
   - 服务状态：enabled（已启用），inactive（未启动）

3. **启动服务**
   ```bash
   sudo make start
   ```
   
   **Expected Output**:
   ```
   Service started.
   ```
   
   **Verify**:
   ```bash
   sudo make status
   ```
   
   **Expected**:
   ```
   ● stunserver.service - STUN Server
      Active: active (running) since ...
   ```
   
   **Test STUN Service**:
   ```bash
   # 使用 netcat 测试 UDP 端口
   echo "" | nc -u -w1 localhost 3478
   
   # 或检查端口监听
   sudo netstat -tulnp | grep 3478
   ss -tulnp | grep 3478
   ```
   
   **Expected**: 端口 3478 UDP/TCP 正在监听

4. **检查日志**
   ```bash
   # 查看 systemd 日志
   sudo journalctl -u stunserver -n 50
   
   # 查看应用日志文件
   sudo tail -f /opt/stunserver/log/stunserver.log
   ```
   
   **Expected**: 日志显示服务启动信息，无错误

5. **重启服务**
   ```bash
   sudo make restart
   ```
   
   **Expected Output**:
   ```
   Service restarted.
   ```
   
   **Verify**:
   ```bash
   sudo make status
   ```
   
   **Expected**: 服务重新变为 active (running)，PID 改变

6. **停止服务**
   ```bash
   sudo make stop
   ```
   
   **Expected Output**:
   ```
   Service stopped.
   ```
   
   **Verify**:
   ```bash
   sudo make status
   ```
   
   **Expected**: Active: inactive (dead)

7. **卸载服务**
   ```bash
   sudo make uninstall
   ```
   
   **Expected Output**:
   ```
   Uninstalling STUN server...
   Uninstallation complete.
   ```
   
   **Verify**:
   ```bash
   # 检查目录已删除
   ls /opt/stunserver 2>&1
   
   # 检查服务已删除
   systemctl status stunserver
   
   # 检查用户已删除
   id stunserver 2>&1
   ```
   
   **Expected**:
   - /opt/stunserver 目录不存在
   - 服务状态：not found
   - 用户：no such user

### Success Criteria

- ✅ `make install` 成功安装并启用服务
- ✅ `make start` 启动服务，端口 3478 监听
- ✅ `make status` 显示正确的服务状态
- ✅ `make restart` 重启服务
- ✅ `make stop` 停止服务
- ✅ `make uninstall` 完全清理所有文件和配置

---

## Scenario 3: Docker 镜像构建验证

**Goal**: 验证 Docker 镜像构建和运行

**Time**: ~15 分钟

### Steps

1. **构建 Docker 镜像**
   ```bash
   cd /opt/stunserver
   docker build -t stunserver:test -f docker/Dockerfile .
   ```
   
   **Expected Output**:
   ```
   Step 1/15 : FROM rust:1.75-alpine AS builder
   ...
   Successfully built abc123def456
   Successfully tagged stunserver:test
   ```
   
   **Verify**:
   ```bash
   docker images stunserver:test
   ```
   
   **Expected**: 镜像存在，大小 < 200MB

2. **运行容器（基本模式）**
   ```bash
   docker run -d \
     --name stunserver-test \
     -p 3478:3478/udp \
     -p 3478:3478/tcp \
     stunserver:test
   ```
   
   **Expected Output**:
   ```
   abc123def456...
   ```
   
   **Verify**:
   ```bash
   # 检查容器运行状态
   docker ps | grep stunserver-test
   
   # 检查端口映射
   docker port stunserver-test
   
   # 测试 STUN 服务
   echo "" | nc -u -w1 localhost 3478
   ```
   
   **Expected**:
   - 容器状态：Up
   - 端口映射：0.0.0.0:3478->3478/udp, 0.0.0.0:3478->3478/tcp
   - STUN 服务响应

3. **查看容器日志**
   ```bash
   docker logs stunserver-test
   docker logs -f stunserver-test
   ```
   
   **Expected**: 显示服务启动日志，无错误

4. **运行容器（带卷挂载）**
   ```bash
   # 停止并删除旧容器
   docker stop stunserver-test
   docker rm stunserver-test
   
   # 创建配置和日志目录
   mkdir -p ./test-config ./test-logs
   
   # 运行新容器
   docker run -d \
     --name stunserver-test \
     -p 3478:3478/udp \
     -p 3478:3478/tcp \
     -v $(pwd)/test-config/stunserver.conf:/opt/stunserver/etc/stunserver.conf:ro \
     -v $(pwd)/test-logs:/opt/stunserver/log \
     -e STUN_LOG_LEVEL=debug \
     stunserver:test
   ```
   
   **Verify**:
   ```bash
   # 检查日志文件生成
   ls -lh test-logs/
   
   # 查看日志内容
   cat test-logs/stunserver.log
   ```
   
   **Expected**:
   - 日志文件在宿主机可见
   - 日志级别为 debug（环境变量覆盖）

5. **测试环境变量配置**
   ```bash
   # 停止并删除旧容器
   docker stop stunserver-test
   docker rm stunserver-test
   
   # 使用不同端口运行
   docker run -d \
     --name stunserver-test \
     -p 3479:3479/udp \
     -p 3479:3479/tcp \
     -e STUN_PORT=3479 \
     stunserver:test
   ```
   
   **Verify**:
   ```bash
   # 测试新端口
   echo "" | nc -u -w1 localhost 3479
   ```
   
   **Expected**: STUN 服务在 3479 端口响应

6. **清理**
   ```bash
   docker stop stunserver-test
   docker rm stunserver-test
   docker rmi stunserver:test
   rm -rf test-config test-logs
   ```

### Success Criteria

- ✅ Docker 镜像构建成功，大小 < 200MB
- ✅ 容器启动后服务可用
- ✅ 端口映射正确
- ✅ 日志可以通过 docker logs 访问
- ✅ 卷挂载工作正常，日志持久化到宿主机
- ✅ 环境变量可以覆盖配置

---

## Scenario 4: 日志轮转验证

**Goal**: 验证应用内置日志轮转功能

**Time**: ~20 分钟（或使用加速测试）

### Steps

1. **安装并启动服务**
   ```bash
   cd deploy
   sudo make install
   sudo make start
   ```

2. **生成大量日志**
   ```bash
   # 方法 1：使用压力测试工具（如果有）
   # 方法 2：修改配置为 debug 级别，产生更多日志
   sudo sed -i 's/level = "info"/level = "debug"/' /opt/stunserver/etc/stunserver.conf
   sudo make restart
   
   # 方法 3：使用脚本发送大量 STUN 请求
   for i in {1..10000}; do
     echo "" | nc -u -w1 localhost 3478
   done
   ```

3. **监控日志文件大小**
   ```bash
   # 持续监控
   watch -n 5 'ls -lh /opt/stunserver/log/'
   ```
   
   **Expected**: 日志文件大小持续增长

4. **触发日志轮转**
   
   **方法 1：等待文件大小达到 100MB**（耗时较长）
   
   **方法 2：临时修改轮转阈值用于测试**
   ```bash
   # 修改配置，设置较小的轮转阈值（如 1MB）
   sudo sed -i 's/max_file_size_mb = 100/max_file_size_mb = 1/' /opt/stunserver/etc/stunserver.conf
   sudo make restart
   
   # 生成日志直到轮转
   while [ $(stat -c%s /opt/stunserver/log/stunserver.log) -lt 1100000 ]; do
     echo "" | nc -u -w1 localhost 3478
   done
   ```

5. **验证轮转结果**
   ```bash
   ls -lh /opt/stunserver/log/
   ```
   
   **Expected**:
   ```
   stunserver.log      # 新的活跃日志（较小）
   stunserver.log.1    # 轮转后的旧日志（接近 1MB）
   ```

6. **验证日志清理**
   
   **方法：创建多个轮转日志文件，验证旧文件被删除**
   ```bash
   # 创建测试文件
   for i in {1..35}; do
     sudo touch -d "$i days ago" /opt/stunserver/log/stunserver.log.$i
   done
   
   # 等待轮转触发或手动触发
   # ...
   
   # 检查超过 30 天的文件是否被删除
   ls /opt/stunserver/log/
   ```
   
   **Expected**: stunserver.log.31 到 stunserver.log.35 被删除

7. **清理**
   ```bash
   sudo make stop
   sudo make uninstall
   ```

### Success Criteria

- ✅ 日志文件达到大小阈值时自动轮转
- ✅ 轮转后新日志文件创建
- ✅ 旧日志文件按序号递增重命名
- ✅ 超过保留天数的日志自动删除

---

## Scenario 5: 边缘情况验证

**Goal**: 验证边缘情况的处理

**Time**: ~10 分钟

### Test 1: 无 systemd 系统

```bash
# 临时重命名 systemctl
sudo mv /bin/systemctl /bin/systemctl.bak

# 尝试安装
cd deploy
sudo make install 2>&1

# 恢复
sudo mv /bin/systemctl.bak /bin/systemctl
```

**Expected**: 显示错误 "Error: systemd not found"，退出码 1

### Test 2: 非 root 用户

```bash
# 切换到普通用户
su - normaluser

# 尝试安装
cd /opt/stunserver/deploy
make install 2>&1
```

**Expected**: 显示错误 "Error: root privileges required"，退出码 1

### Test 3: 端口被占用

```bash
# 占用 3478 端口
sudo nc -l -u 3478 &

# 尝试启动服务
cd deploy
sudo make start 2>&1

# 清理
sudo kill %1
```

**Expected**: 启动失败，日志显示端口占用错误

### Test 4: 安装目录已存在

```bash
# 创建旧版本目录
sudo mkdir -p /opt/stunserver/bin
sudo echo "old binary" | sudo tee /opt/stunserver/bin/stunserver

# 安装
cd deploy
sudo make install

# 验证
ls -lh /opt/stunserver/bin/stunserver
```

**Expected**: 安装成功，旧文件被覆盖或备份

---

## Validation Summary

### Checklist

| Scenario | Test Case                          | Status | Notes |
|----------|------------------------------------|--------|-------|
| 1        | 开发构建流程                       | ⏳     |       |
| 2        | 安装与部署流程                     | ⏳     |       |
| 3        | Docker 镜像构建                    | ⏳     |       |
| 4        | 日志轮转                           | ⏳     |       |
| 5        | 边缘情况                           | ⏳     |       |

### Success Metrics

| Metric                          | Target       | Actual | Status |
|---------------------------------|--------------|--------|--------|
| 构建时间                        | < 5 min      | -      | ⏳     |
| 安装时间                        | < 5 min      | -      | ⏳     |
| Docker 镜像大小                 | < 200 MB     | -      | ⏳     |
| Docker 镜像构建时间             | < 10 min     | -      | ⏳     |
| 容器启动到服务可用              | < 30 sec     | -      | ⏳     |
| 所有测试通过                    | 100%         | -      | ⏳     |

---

## Troubleshooting

### Problem: make install 失败 "systemd not found"

**Solution**: 
```bash
# 检查 systemd
ps -p 1 -o comm=

# 如果是 init 或其他，说明不是 systemd 系统
# 本功能仅支持 systemd
```

### Problem: 服务启动失败 "Address already in use"

**Solution**:
```bash
# 检查端口占用
sudo netstat -tulnp | grep 3478

# 杀死占用进程
sudo kill -9 <PID>

# 或修改配置使用其他端口
sudo vim /opt/stunserver/etc/stunserver.conf
# 修改 port = 3479
```

### Problem: Docker 构建失败 "cargo build returned non-zero"

**Solution**:
```bash
# 检查本地构建是否成功
make build

# 清理 Docker 缓存
docker builder prune

# 重新构建
docker build --no-cache -t stunserver:test -f docker/Dockerfile .
```

### Problem: 日志轮转未触发

**Solution**:
```bash
# 检查配置
cat /opt/stunserver/etc/stunserver.conf | grep max_file_size_mb

# 检查日志大小
ls -lh /opt/stunserver/log/

# 手动触发（如果支持）
# 或等待下一次检查周期
```

---

## Next Steps

完成所有验证场景后：

1. 更新 Validation Summary 表格中的 Status
2. 记录 Actual 指标值
3. 如果有失败项，记录问题并修复
4. 所有测试通过后，可以进入实现阶段（`/speckit-tasks`）
