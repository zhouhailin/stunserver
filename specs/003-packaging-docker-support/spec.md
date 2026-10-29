# Feature Specification: 打包优化与 Docker 镜像支持

**Feature Branch**: `003-packaging-docker-support`

**Created**: 2026-10-29

**Status**: Draft

**Input**: User description: "优化打包、增加docker镜像。当前目录增加 Makefile，需要包含常用命令。打包后的目录结构需要调整 bin log etc 目录，并且在linux平台下增加Makefile，可以make install/uninstall/start/stop/restart ，install 默认systemctl enable ，uninstall system disable。修改 systemd/stunserver.service 文件 修改 https://github.com/stunserver/stunserver 地址 https://github.com/zhouhailin/stunserver 。增加针对 stunserver 增加docker image 构建"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 简化部署流程 (Priority: P1)

系统管理员需要快速部署 STUN 服务器。通过标准化的打包结构和 Makefile 命令，管理员可以在 Linux 系统上一键完成安装、启动、停止和卸载操作，无需手动配置 systemd 服务或复制文件。

**Why this priority**: 部署是用户使用软件的第一步，如果部署复杂会直接影响用户体验和采用率。标准化的安装流程减少了人为错误，提高了部署效率。

**Independent Test**: 可以通过在干净的 Linux 系统上执行 `make install` 和 `make start` 来测试，验证服务是否正确安装并启动，无需其他功能支持。

**Acceptance Scenarios**:

1. **Given** 系统管理员在 Linux 服务器上，**When** 执行 `make install`，**Then** 系统自动将二进制文件安装到标准目录，创建必要的日志和配置目录，并启用 systemd 服务
2. **Given** 服务已安装，**When** 执行 `make start`，**Then** STUN 服务器启动并可以通过 systemctl status 查看运行状态
3. **Given** 服务正在运行，**When** 执行 `make stop`，**Then** STUN 服务器优雅停止，所有连接正常关闭
4. **Given** 服务已安装，**When** 执行 `make restart`，**Then** 服务先停止后启动，配置重新加载
5. **Given** 服务已安装，**When** 执行 `make uninstall`，**Then** 系统禁用并移除 systemd 服务，清理所有安装的文件和目录

---

### User Story 2 - 容器化部署 (Priority: P2)

DevOps 工程师需要在容器环境中部署 STUN 服务器。通过提供预构建的 Docker 镜像，工程师可以快速在 Kubernetes、Docker Swarm 或其他容器编排平台中部署服务，享受容器化带来的隔离性、可移植性和易于扩展的优势。

**Why this priority**: 容器化是现代应用部署的标准方式，支持 Docker 镜像可以大幅降低在云环境中的部署门槛，但不影响传统部署方式的核心功能。

**Independent Test**: 可以通过 `docker build` 构建镜像，然后使用 `docker run` 启动容器，验证 STUN 服务在容器中正常运行并可访问。

**Acceptance Scenarios**:

1. **Given** 项目源码和 Dockerfile，**When** 执行 `docker build`，**Then** 成功构建包含 STUN 服务器的 Docker 镜像
2. **Given** Docker 镜像已构建，**When** 执行 `docker run` 启动容器，**Then** STUN 服务器在容器中正常运行，监听配置的端口
3. **Given** 容器正在运行，**When** 检查容器日志，**Then** 可以看到 STUN 服务器的运行日志和请求处理记录
4. **Given** 需要自定义配置，**When** 通过环境变量或挂载配置文件启动容器，**Then** 容器内的 STUN 服务器使用自定义配置运行
5. **Given** 开发者推送代码到 main 分支，**When** GitHub Actions 工作流自动触发，**Then** 成功构建 Docker 镜像并推送到 ghcr.io，标签为 latest
6. **Given** 维护者创建一个新的 release 或 tag（如 v0.1.1），**When** GitHub Actions 工作流自动触发，**Then** 成功构建 Docker 镜像并推送到 ghcr.io，标签为 v0.1.1 和 latest
7. **Given** 镜像已推送到 ghcr.io，**When** 用户执行 `docker pull ghcr.io/zhouhailin/stunserver:v0.1.1`，**Then** 成功下载指定版本的镜像

---

### User Story 3 - 标准化目录结构 (Priority: P3)

系统集成员需要将 STUN 服务器集成到现有的运维体系中。通过遵循 Linux 文件系统层次结构标准（bin、log、etc），集成员可以更容易地将 STUN 服务器纳入现有的监控、日志收集、配置管理等系统中。

**Why this priority**: 标准化的目录结构提高了与其他系统的兼容性，但不会直接影响核心功能的可用性。

**Independent Test**: 可以通过检查安装后的目录结构，验证二进制文件、配置文件和日志文件是否分别位于标准目录中。

**Acceptance Scenarios**:

1. **Given** 执行安装操作，**When** 检查安装目录，**Then** 二进制文件位于 bin 目录，配置文件位于 etc 目录，日志文件位于 log 目录
2. **Given** 服务运行中，**When** 检查日志目录，**Then** 日志文件按标准格式命名并持续写入
3. **Given** 需要修改配置，**When** 编辑 etc 目录中的配置文件，**Then** 配置更改在下次启动时生效

---

### User Story 4 - 开发便利性 (Priority: P4)

开发者需要在本地快速构建和测试 STUN 服务器。通过项目根目录的 Makefile，开发者可以使用统一的命令完成编译、测试、打包等常见操作，无需记忆复杂的构建命令。

**Why this priority**: 开发便利性提高了开发效率，但属于辅助功能，不影响最终用户的部署和使用。

**Independent Test**: 可以通过在项目根目录执行 `make build` 和 `make test` 来验证编译和测试功能是否正常工作。

**Acceptance Scenarios**:

1. **Given** 开发者在项目目录，**When** 执行 `make build`，**Then** 系统编译生成 STUN 服务器二进制文件
2. **Given** 代码已修改，**When** 执行 `make test`，**Then** 运行所有单元测试和集成测试，报告测试结果
3. **Given** 需要发布新版本，**When** 执行 `make package`，**Then** 生成包含所有必要文件的分发包
4. **Given** 开发者首次接触项目，**When** 阅读 README.md，**Then** 能够快速了解项目功能、安装方法、Docker 使用、开发命令等完整信息
5. **Given** 中文用户访问项目，**When** 阅读 README.zh-CN.md，**Then** 能够获得与英文版一致的完整中文文档

---

### Edge Cases

- 当系统没有安装 systemd 时，install 命令检测 systemd 可用性，如果不存在则报错退出并提供安装指导（仅支持 systemd 系统）
- 当目标目录已存在旧版本文件时，install 命令应该如何处理？应该备份或提示用户确认覆盖
- 当端口被占用时，服务启动失败如何处理？应该记录清晰的错误日志并提示用户检查端口配置
- 当 Docker 守护进程未运行时，docker 相关命令如何处理？应该检测 Docker 状态并提供启动指导
- 当用户没有 root 权限时，install 命令如何处理？应该在开始时检查权限并提示用户使用 sudo

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 系统 MUST 在项目根目录提供 Makefile，包含 build、test、package、clean 等常用开发命令
- **FR-002**: 系统 MUST 在 deploy 目录提供独立的安装 Makefile（deploy/Makefile），支持 install、uninstall、start、stop、restart 命令
- **FR-003**: install 命令 MUST 将二进制文件安装到 /opt/stunserver/bin 目录，配置文件到 /opt/stunserver/etc，日志到 /opt/stunserver/log
- **FR-004**: install 命令 MUST 创建配置目录（etc）和日志目录（log），并设置适当的权限
- **FR-005**: install 命令 MUST 默认启用 systemd 服务（systemctl enable），使服务在系统启动时自动运行
- **FR-006**: uninstall 命令 MUST 禁用 systemd 服务（systemctl disable）并移除所有安装的文件和目录
- **FR-007**: start 命令 MUST 启动 STUN 服务器服务，并可通过 systemctl status 查看运行状态
- **FR-008**: stop 命令 MUST 优雅停止 STUN 服务器，确保所有连接正常关闭
- **FR-009**: restart 命令 MUST 先停止后启动服务，实现配置重新加载
- **FR-010**: 系统 MUST 提供 systemd 服务配置文件（stunserver.service），定义服务的启动行为和依赖关系
- **FR-011**: systemd 服务配置文件中的项目地址 MUST 更新为 https://github.com/zhouhailin/stunserver
- **FR-012**: 系统 MUST 提供 Dockerfile，用于构建 STUN 服务器的 Docker 镜像
- **FR-013**: Docker 镜像 MUST 包含运行 STUN 服务器所需的所有依赖和配置文件
- **FR-014**: Docker 镜像 MUST 支持通过环境变量或挂载方式自定义配置
- **FR-015**: Docker 镜像 MUST 暴露 STUN 服务监听的标准端口（3478 UDP/TCP）
- **FR-016**: 打包后的目录结构 MUST 遵循 Linux 文件系统层次标准，包含 bin、log、etc 三个主要目录
- **FR-017**: 系统 MUST 提供 GitHub Actions 工作流文件（.github/workflows/docker-publish.yml），实现 Docker 镜像的自动化构建和发布
- **FR-018**: GitHub Actions 工作流 MUST 在 push 到 main 分支时触发构建，并将镜像标记为 latest
- **FR-019**: GitHub Actions 工作流 MUST 在创建 release 或 tag 时触发构建，并将镜像标记为语义化版本号（如 v0.1.1）和 latest
- **FR-020**: GitHub Actions 工作流 MUST 将构建的镜像推送到 GitHub Container Registry (ghcr.io)
- **FR-021**: GitHub Actions 工作流 MUST 使用 GITHUB_TOKEN 进行容器镜像仓库认证，无需额外配置密钥
- **FR-022**: Docker 镜像标签 MUST 遵循语义化版本规范，格式为 vX.Y.Z（如 v0.1.1），并与 Git tag 保持一致
- **FR-023**: 系统 MUST 更新项目根目录的 README.md（英文版本），包含所有新增功能的文档：Makefile 使用说明、Docker 镜像构建和运行、GitHub Actions CI/CD 工作流、安装部署指南、配置说明等
- **FR-024**: 系统 MUST 创建中文版本的 README 文件（README.zh-CN.md），内容与英文版本保持一致，为中文用户提供完整的项目文档

### Key Entities *(include if feature involves data)*

- **Makefile**: 定义构建、测试、打包、安装、卸载等自动化命令的配置文件，分为开发用 Makefile（根目录）和部署用 Makefile（deploy/Makefile）
- **systemd 服务配置**: 定义 STUN 服务器作为系统服务运行的行为，包括启动命令、依赖关系、日志输出等
- **Docker 镜像**: 包含 STUN 服务器运行时环境的容器镜像，包括二进制文件、配置文件、依赖库等
- **GitHub Actions 工作流**: 自动化构建和发布 Docker 镜像的 CI/CD 管道，配置在 .github/workflows/docker-publish.yml
- **配置文件**: /opt/stunserver/etc/stunserver.conf，TOML 格式，包含端口、日志级别、监听地址等所有配置项
- **目录结构**: 标准化的文件组织方式，bin 目录存放可执行文件，etc 目录存放 stunserver.conf，log 目录存放日志文件

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 系统管理员可以在 5 分钟内完成 STUN 服务器的安装和启动，无需查阅额外文档
- **SC-002**: 使用 `make install` 安装后，服务在系统重启后自动启动，无需手动干预
- **SC-003**: Docker 镜像构建成功率达到 100%，构建时间在 10 分钟内完成
- **SC-004**: 容器启动后，STUN 服务在 30 秒内可用并响应请求
- **SC-005**: 所有 Makefile 命令（install、uninstall、start、stop、restart）在主流 Linux 发行版（Ubuntu 20.04+、CentOS 7+、Debian 10+）上测试通过
- **SC-006**: 卸载操作完全清理所有安装文件，不留下残留配置或数据
- **SC-007**: STUN 服务器内置日志轮转功能，单个日志文件不超过 100MB 时自动轮转，保留最近 30 天的日志，无需外部工具（如 logrotate）
- **SC-008**: Docker 镜像大小不超过 200MB，便于快速拉取和部署
- **SC-009**: GitHub Actions 工作流在 push 到 main 分支后 15 分钟内完成镜像构建和发布
- **SC-010**: GitHub Actions 工作流在创建 release/tag 后 15 分钟内完成版本镜像构建和发布
- **SC-011**: 从 ghcr.io 拉取镜像的时间不超过 5 分钟（取决于网络条件，以 100Mbps 连接为基准）

## Clarifications

### Session 2026-10-29

- Q: 安装路径应该选择哪个标准位置？ → A: /opt/stunserver — 第三方应用标准路径，所有文件集中管理（bin/log/etc 子目录）
- Q: Makefile 应该采用单一文件还是分离结构？ → A: 分离 Makefile — 根目录 Makefile 用于开发（build/test/package/clean），deploy/Makefile 用于部署操作（install/uninstall/start/stop/restart）
- Q: 配置目录（etc）应该包含哪些配置文件？ → A: 单一配置文件 — /opt/stunserver/etc/stunserver.conf（TOML 格式），包含所有配置项（端口、日志级别、监听地址等）
- Q: 日志轮转应该采用什么机制？ → A: 应用内置 — STUN 服务器内部实现日志轮转逻辑，自动按大小（100MB）和时间轮转，保留最近 30 天日志
- Q: 对于不支持 systemd 的 Linux 系统应该如何处理？ → A: 仅支持 systemd — install 命令检测 systemd，不存在则报错退出并提供安装指导
- Q: GitHub Actions 工作流应该在什么事件触发时自动构建 Docker 镜像？ → A: push 到 main 分支 + 创建 release/tag — 主分支保持最新，release 时打版本标签
- Q: 构建的 Docker 镜像应该推送到哪个容器镜像仓库？ → A: GitHub Container Registry (ghcr.io) — 与 GitHub 原生集成，认证简单，权限管理统一
- Q: Docker 镜像应该采用什么标签策略？ → A: 语义化版本标签（v0.1.1） + latest — 支持版本锁定，同时提供最新镜像引用
- 文档需求：更新 README.md（英文）并创建 README.zh-CN.md（中文），包含所有新增功能的完整文档

## Assumptions

- 目标部署环境为 Linux 操作系统，支持 systemd 服务管理器
- 用户具有 root 权限或可以使用 sudo 执行安装操作
- 系统已安装 Docker 引擎（版本 20.10 或更高）用于容器化部署
- STUN 服务器监听的默认端口为 3478（UDP/TCP），用户可根据需要修改配置
- 日志目录和配置目录使用 /opt/stunserver 作为基础路径，符合第三方应用标准
- 分发包包含所有必要的文件，用户无需额外下载依赖或配置
- 现有 systemd 服务配置文件需要更新 GitHub 仓库地址，从原地址迁移到新的维护者地址
- Makefile 命令遵循 Linux 标准命令习惯，如 install、uninstall、start、stop、restart 等
- Docker 镜像基于轻量级基础镜像（如 Alpine Linux）以减小镜像体积
