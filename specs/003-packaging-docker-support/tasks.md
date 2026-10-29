# Tasks: 打包优化与 Docker 镜像支持

**Input**: Design documents from `/specs/003-packaging-docker-support/`

**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Tests**: Test tasks are NOT included (not explicitly requested in specification). Follow TDD principles during implementation per constitution.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic directory structure

- [x] T001 Create deploy/ directory structure per plan.md project layout
- [x] T002 Create docker/ directory structure per plan.md project layout
- [x] T003 [P] Add toml = "0.8" dependency to Cargo.toml workspace dependencies
- [x] T004 [P] Add serde derive feature to existing serde dependency in Cargo.toml (verify already present)

**Checkpoint**: Basic directory structure ready, dependencies configured

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T005 Update systemd/stunserver.service: change Documentation URL from https://github.com/stunserver/stunserver to https://github.com/zhouhailin/stunserver per FR-011
- [x] T006 Update systemd/stunserver.service: change ExecStart path from /usr/local/bin/stunserver to /opt/stunserver/bin/stunserver per FR-003
- [x] T007 Update systemd/stunserver.service: change config path from /etc/stunserver/config.yaml to /opt/stunserver/etc/stunserver.conf per data-model.md
- [x] T008 Update systemd/stunserver.service: change ReadWritePaths from /var/log/stunserver to /opt/stunserver/log per FR-003
- [x] T009 [P] Create deploy/stunserver.conf.example with complete TOML schema per data-model.md Entity 1 (all sections: server, logging, network, security with defaults and validation rules)
- [x] T010 Implement configuration parsing module in src/server/src/config.rs: define Config, ServerConfig, LoggingConfig, NetworkConfig, SecurityConfig structs with serde Deserialize derive per data-model.md Rust Data Structure
- [x] T011 Implement configuration validation logic in src/server/src/config.rs: validate all fields per data-model.md Validation Rules table (port range 1-65535, log level enum, path existence, etc.)
- [x] T012 Implement log rotation module in src/server/src/logging.rs: define RotatingLogger struct with current_file, current_size, max_size (100MB default), retention_days (30 default), log_dir fields per data-model.md Entity 3
- [x] T013 Implement log rotation trigger logic in src/server/src/logging.rs: check file size >= max_file_size_mb OR date change (midnight) per research.md log rotation implementation strategy
- [x] T014 Implement log rotation algorithm in src/server/src/logging.rs: close current file, rename with incrementing numbers (stunserver.log.1, .2, etc.), create new file, cleanup files older than retention_days per data-model.md Rotation Algorithm
- [x] T015 Integrate configuration loading in src/server/src/main.rs: load /opt/stunserver/etc/stunserver.conf at startup, parse with toml crate, apply validation per T010-T011
- [x] T016 Integrate RotatingLogger in src/server/src/main.rs: initialize logger with config values, replace or wrap existing env_logger setup per T012-T014

**Checkpoint**: Foundation ready - configuration parsing and log rotation implemented, systemd service file updated. User story implementation can now begin.

---

## Phase 3: User Story 1 - 简化部署流程 (Priority: P1) 🎯 MVP

**Goal**: 系统管理员可以通过 make install/start/stop/restart/uninstall 命令快速部署和管理 STUN 服务器服务

**Independent Test**: 在干净的 Linux 系统上执行 `cd deploy && sudo make install && sudo make start`，验证服务正确安装、启动、监听 3478 端口，然后执行 `sudo make stop && sudo make uninstall` 验证完全清理

### Implementation for User Story 1

- [x] T017 [US1] Create deploy/Makefile with .PHONY targets (install, uninstall, start, stop, restart, status, help) and variable definitions (INSTALL_DIR=/opt/stunserver, SERVICE_NAME=stunserver) per contracts/makefile-commands.md Contract 2
- [x] T018 [US1] Implement install target in deploy/Makefile: check root permissions (id -u), check systemd availability (command -v systemctl), create /opt/stunserver/{bin,etc,log} directories, copy binary from ../target/release/stunserver to /opt/stunserver/bin/, copy stunserver.conf.example to /opt/stunserver/etc/stunserver.conf, copy ../systemd/stunserver.service to /etc/systemd/system/, run systemctl daemon-reload, create stunserver system user (useradd -r -s /bin/false), set ownership (chown -R stunserver:stunserver /opt/stunserver), run systemctl enable stunserver per contracts/makefile-commands.md install command spec
- [x] T019 [US1] Implement uninstall target in deploy/Makefile: check root permissions, run systemctl stop stunserver (ignore errors), run systemctl disable stunserver (ignore errors), remove /etc/systemd/system/stunserver.service, run systemctl daemon-reload, remove /opt/stunserver directory recursively, optionally remove stunserver system user (userdel) per contracts/makefile-commands.md uninstall command spec
- [x] T020 [US1] Implement start target in deploy/Makefile: check root permissions, run systemctl start stunserver, verify service status per contracts/makefile-commands.md start command spec
- [x] T021 [US1] Implement stop target in deploy/Makefile: check root permissions, run systemctl stop stunserver per contracts/makefile-commands.md stop command spec
- [x] T022 [US1] Implement restart target in deploy/Makefile: check root permissions, run systemctl restart stunserver per contracts/makefile-commands.md restart command spec
- [x] T023 [US1] Implement status target in deploy/Makefile: run systemctl status stunserver, display service information per contracts/makefile-commands.md status command spec
- [x] T024 [US1] Implement help target in deploy/Makefile: display all available commands with descriptions per contracts/makefile-commands.md help command spec
- [x] T025 [US1] Add error handling to deploy/Makefile install target: detect missing systemd and exit with error message "Error: systemd not found. This installer only supports systemd-based systems." and exit code 1 per contracts/makefile-commands.md install error handling
- [x] T026 [US1] Add error handling to deploy/Makefile install/uninstall targets: detect non-root user and exit with error message "Error: root privileges required. Use sudo." and exit code 1 per contracts/makefile-commands.md permission model
- [x] T027 [US1] Add idempotency to deploy/Makefile install target: check if /opt/stunserver exists, backup or prompt for confirmation before overwriting per research.md Makefile design pattern edge case handling
- [x] T028 [US1] Test install/uninstall flow manually: run `cd deploy && sudo make install`, verify directory structure, verify service enabled, run `sudo make uninstall`, verify cleanup per quickstart.md Scenario 2

**Checkpoint**: User Story 1 complete - administrators can install, start, stop, restart, and uninstall STUN server with simple make commands. MVP ready for deployment.

---

## Phase 4: User Story 2 - 容器化部署 (Priority: P2)

**Goal**: DevOps 工程师可以通过 Docker 镜像快速部署 STUN 服务器，支持环境变量配置和卷挂载持久化

**Independent Test**: 执行 `docker build -t stunserver:test -f docker/Dockerfile .`，验证镜像大小 < 200MB，然后执行 `docker run -d -p 3478:3478/udp -p 3478:3478/tcp stunserver:test`，验证容器启动后 30 秒内服务可用

### Implementation for User Story 2

- [x] T029 [US2] Create docker/Dockerfile with multi-stage build: Stage 1 (builder) uses rust:1.75-alpine, copies Cargo.toml/Cargo.lock first for cache optimization, then copies src/ and crates/, runs cargo build --release; Stage 2 (runtime) uses alpine:3.19, installs ca-certificates, sets WORKDIR /opt/stunserver, copies binary from builder, copies deploy/stunserver.conf.example to /opt/stunserver/etc/stunserver.conf, creates log directory with 755 permissions, EXPOSE 3478/udp 3478/tcp, sets ENTRYPOINT and CMD per research.md Docker image build strategy and contracts/docker-interface.md Contract 1
- [x] T030 [US2] Create docker/docker-entrypoint.sh: make executable (chmod +x), check for environment variables (STUN_CONFIG_PATH, STUN_LOG_LEVEL, STUN_PORT, STUN_LISTEN_ADDRESS), generate or copy configuration file, exec stunserver with config path per contracts/docker-interface.md Contract 2 entrypoint and CMD section
- [x] T031 [US2] Create .dockerignore at repository root: exclude target/, .git/, .github/, docs/, specs/, *.md (except README.md), .specify/, .qoder/ per research.md Docker image build strategy build context section
- [x] T032 [US2] Add HEALTHCHECK instruction to docker/Dockerfile: use nc -z localhost 3478 or similar, set interval=30s, timeout=10s, start-period=5s, retries=3 per contracts/docker-interface.md Contract 2 health check section
- [x] T033 [US2] Implement environment variable override logic in docker-entrypoint.sh: parse STUN_LOG_LEVEL, STUN_PORT, STUN_LISTEN_ADDRESS, STUN_CONFIG_PATH, apply to configuration or pass as command-line arguments, ensure precedence (env vars > config file > defaults) per contracts/docker-interface.md Contract 2 environment variables table
- [x] T034 [US2] Test Docker build: run `docker build -t stunserver:test -f docker/Dockerfile .`, verify image size < 200MB (check with `docker images`), verify build completes in < 10 minutes per quickstart.md Scenario 3 step 1 and success criteria SC-003, SC-008
- [x] T035 [US2] Test Docker run basic mode: run `docker run -d --name stunserver-test -p 3478:3478/udp -p 3478:3478/tcp stunserver:test`, verify container status (docker ps), verify port mapping (docker port), test STUN service (nc -u -w1 localhost 3478), check logs (docker logs stunserver-test) per quickstart.md Scenario 3 steps 2-3
- [x] T036 [US2] Test Docker run with volume mounts: run container with `-v $(pwd)/test-logs:/opt/stunserver/log`, verify log files created on host, test log rotation by generating logs per quickstart.md Scenario 3 step 4
- [x] T037 [US2] Test Docker environment variable overrides: run container with `-e STUN_PORT=3479 -p 3479:3479/udp -p 3479:3479/tcp`, verify service listens on custom port per quickstart.md Scenario 3 step 5

**Checkpoint**: User Story 2 complete - DevOps engineers can build and run STUN server in Docker with customizable configuration via environment variables and persistent logs via volume mounts.

---

## Phase 5: User Story 3 - 标准化目录结构 (Priority: P3)

**Goal**: 系统集成员可以依赖标准化的 /opt/stunserver/{bin,etc,log} 目录结构将 STUN 服务器集成到现有运维体系

**Independent Test**: 执行安装后检查目录结构，验证二进制文件在 bin/，配置文件在 etc/，日志文件在 log/，验证文件权限符合标准

**Note**: 目录结构的创建已在 User Story 1 (deploy/Makefile install target) 中实现。本阶段主要验证和文档化。

### Implementation for User Story 3

- [x] T038 [US3] Verify directory structure after install: run `sudo make install`, check /opt/stunserver/bin/stunserver exists and is executable (755), check /opt/stunserver/etc/stunserver.conf exists and is readable (640), check /opt/stunserver/log/ exists and is writable (750) per data-model.md Entity 2 permissions table
- [x] T039 [US3] Verify file ownership after install: check all files owned by stunserver:stunserver, check /opt/stunserver/ root directory is 755, check etc/ is 750 (restricted access) per data-model.md Entity 2 permissions table
- [x] T040 [US3] Test configuration file editing: modify /opt/stunserver/etc/stunserver.conf (change port or log level), restart service, verify new configuration applied per quickstart.md Scenario 1 step 3 acceptance scenario
- [x] T041 [US3] Test log file location: start service, generate some traffic, verify logs written to /opt/stunserver/log/stunserver.log, verify log rotation creates stunserver.log.1, .2, etc. per data-model.md Entity 3 file naming convention
- [x] T042 [US3] Document directory structure in README.md or deploy/README.md: explain /opt/stunserver/{bin,etc,log} layout, file permissions, ownership, and how to customize paths (if supported) per data-model.md Entity 2 installation layout

**Checkpoint**: User Story 3 complete - directory structure follows Linux filesystem hierarchy standard, integrated with existing运维 systems.

---

## Phase 6: User Story 4 - 开发便利性 (Priority: P4)

**Goal**: 开发者可以通过根目录 Makefile 使用统一命令完成编译、测试、打包等常见操作

**Independent Test**: 在项目根目录执行 `make build && make test && make package`，验证所有命令正常工作

### Implementation for User Story 4

- [x] T043 [US4] Create root Makefile with .PHONY targets (all, build, test, package, clean, fmt, lint, help) and default target (all: build) per contracts/makefile-commands.md Contract 1
- [x] T044 [US4] Implement build target in root Makefile: run `cargo build --release`, display compilation progress per contracts/makefile-commands.md build command spec
- [x] T045 [US4] Implement test target in root Makefile: run `cargo test` and `cargo test -- --ignored`, display test results summary per contracts/makefile-commands.md test command spec
- [x] T046 [US4] Implement package target in root Makefile: check binary exists (target/release/stunserver), read version from Cargo.toml or VERSION env var, create dist/stunserver-{version}/ directory structure (bin/, etc/, log/), copy binary, config example, systemd service, create tar.gz archive per contracts/makefile-commands.md package command spec
- [x] T047 [US4] Implement clean target in root Makefile: run `cargo clean`, remove dist/ directory per contracts/makefile-commands.md clean command spec
- [x] T048 [US4] Implement fmt target in root Makefile: run `cargo fmt --all` per contracts/makefile-commands.md fmt command spec
- [x] T049 [US4] Implement lint target in root Makefile: run `cargo clippy -- -D warnings`, exit with error if warnings found per contracts/makefile-commands.md lint command spec
- [x] T050 [US4] Implement help target in root Makefile: display all available commands with descriptions per contracts/makefile-commands.md help command spec
- [x] T051 [US4] Test development workflow: run `make build`, verify binary created; run `make test`, verify all tests pass; run `make fmt`, verify code formatted; run `make lint`, verify no warnings; run `make package`, verify dist/ created with correct structure per quickstart.md Scenario 1

**Checkpoint**: User Story 4 complete - developers have unified command interface for common development tasks.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories, final validation, and documentation

- [x] T052 [P] Create comprehensive README.md section on packaging and deployment: document Makefile commands (both root and deploy/), Docker usage, installation paths, configuration file format, log rotation behavior, systemd integration
- [x] T053 [P] Add example docker-compose.yml in docker/ directory: define stunserver service with ports, volumes, environment variables, healthcheck, restart policy per contracts/docker-interface.md Contract 3
- [x] T054 [P] Create CHANGELOG.md entry for this feature: document all additions (Makefile, Docker support, systemd updates, log rotation, directory structure)
- [x] T055 Run quickstart.md Scenario 1 (development build flow) end-to-end and verify all steps pass
- [x] T056 Run quickstart.md Scenario 2 (installation and deployment flow) end-to-end and verify all steps pass
- [x] T057 Run quickstart.md Scenario 3 (Docker image build and run) end-to-end and verify all steps pass
- [x] T058 Run quickstart.md Scenario 4 (log rotation verification) end-to-end and verify all steps pass
- [x] T059 Run quickstart.md Scenario 5 (edge case testing) and verify error handling works correctly
- [x] T060 Verify all success criteria from spec.md: SC-001 (install < 5 min), SC-002 (auto-start on reboot), SC-003 (Docker build < 10 min), SC-004 (container ready < 30 sec), SC-005 (all commands work on Ubuntu/CentOS/Debian), SC-006 (clean uninstall), SC-007 (log rotation 100MB/30 days), SC-008 (Docker image < 200MB)
- [x] T061 Code review and cleanup: ensure all code passes `cargo fmt --check` and `cargo clippy -- -D warnings`, remove any debug code or temporary files
- [x] T062 Update .gitignore: add dist/, *.tar.gz, test-logs/, test-config/ to prevent accidental commits
- [x] T063 [P] Update README.md with comprehensive documentation: project overview, features, installation guide (make install/start/stop/restart/uninstall), Docker usage (build, run, environment variables, volumes), GitHub Actions CI/CD workflow, configuration file format (TOML), directory structure (/opt/stunserver/{bin,etc,log}), log rotation behavior, troubleshooting section per FR-023
- [x] T064 [P] Create README.zh-CN.md with complete Chinese translation: mirror all sections from English README.md, ensure technical terms are accurately translated, maintain consistent formatting and structure per FR-024
- [x] T065 Add cross-references between README.md and README.zh-CN.md: add language switch links at the top of both files (e.g., "[中文文档](README.zh-CN.md)" in English version, "[English](README.md)" in Chinese version)
- [x] T066 Verify README accuracy: test all commands and examples documented in README.md and README.zh-CN.md to ensure they work as described

**Checkpoint**: All polish tasks complete, feature ready for release.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3-6)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2 → P3 → P4)
- **Polish (Phase 7)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories - **MVP**
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - Independent of US1, but benefits from US1 testing
- **User Story 3 (P3)**: Can start after US1 (Phase 3) - Directory structure created by US1 install target
- **User Story 4 (P4)**: Can start after Foundational (Phase 2) - Independent of other stories

### Within Each User Story

- Models before services (where applicable)
- Core implementation before integration
- Story complete before moving to next priority
- Tests (if added later) MUST be written and FAIL before implementation

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel: T003, T004
- All Foundational tasks marked [P] can run in parallel: T009, T005-T008 (sequential within systemd update)
- Once Foundational phase completes:
  - US1 (T017-T028) can start
  - US2 (T029-T037) can start in parallel with US1
  - US4 (T043-T051) can start in parallel with US1 and US2
- All tasks within a user story marked [P] can run in parallel
- Polish tasks marked [P] can run in parallel: T052, T053, T054, T063, T064

---

## Parallel Example: User Story 1

```bash
# After Foundational phase completes, launch User Story 1:
Task T017: "Create deploy/Makefile with .PHONY targets"

# Then sequentially implement targets (each depends on previous):
Task T018: "Implement install target"
Task T019: "Implement uninstall target"
Task T020: "Implement start target"
Task T021: "Implement stop target"
Task T022: "Implement restart target"
Task T023: "Implement status target"
Task T024: "Implement help target"

# Then add error handling (can be parallel):
Task T025: "Add systemd detection error handling" [P]
Task T026: "Add root permission error handling" [P]
Task T027: "Add idempotency to install" [P]

# Finally test:
Task T028: "Test install/uninstall flow"
```

---

## Parallel Example: User Story 2

```bash
# After Foundational phase completes, launch User Story 2:
# These can all run in parallel:
Task T029: "Create docker/Dockerfile" [P]
Task T030: "Create docker/docker-entrypoint.sh" [P]
Task T031: "Create .dockerignore" [P]

# Then sequentially:
Task T032: "Add HEALTHCHECK to Dockerfile"
Task T033: "Implement environment variable override logic"

# Then test (
Task T034: "Test Docker build"
Task T035: "Test Docker run basic mode"
Task T036: "Test Docker run with volume mounts"
Task T037: "Test Docker environment variable overrides"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T004)
2. Complete Phase 2: Foundational (T005-T016) - **CRITICAL - blocks all stories**
3. Complete Phase 3: User Story 1 (T017-T028)
4. **STOP and VALIDATE**: Test User Story 1 independently per quickstart.md Scenario 2
5. Deploy/demo if ready - **MVP ACHIEVED**

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 (P1) → Test independently → Deploy/Demo (**MVP!**)
3. Add User Story 2 (P2) → Test independently → Deploy/Demo
4. Add User Story 3 (P3) → Test independently → Deploy/Demo
5. Add User Story 4 (P4) → Test independently → Deploy/Demo
6. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (P1 - MVP)
   - Developer B: User Story 2 (P2 - Docker)
   - Developer C: User Story 4 (P4 - Dev Makefile)
3. After US1 complete, one developer can handle US3 (validation tasks)
4. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- **TDD Reminder**: Per constitution, write tests FIRST for each implementation task, verify they fail, then implement
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- User Story 3 is mostly validation since directory structure is created by US1
- Log rotation (T012-T014, T016) is in Foundational phase because it's a core server feature needed by all deployment scenarios
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence

---

## Task Summary

**Total Tasks**: 66

**By Phase**:
- Phase 1 (Setup): 4 tasks
- Phase 2 (Foundational): 12 tasks
- Phase 3 (US1 - 简化部署流程): 12 tasks 🎯 **MVP**
- Phase 4 (US2 - 容器化部署): 9 tasks
- Phase 5 (US3 - 标准化目录结构): 5 tasks
- Phase 6 (US4 - 开发便利性): 9 tasks
- Phase 7 (Polish): 15 tasks

**By User Story**:
- US1: 12 tasks (T017-T028)
- US2: 9 tasks (T029-T037)
- US3: 5 tasks (T038-T042)
- US4: 9 tasks (T043-T051)
- Cross-cutting: 31 tasks (T001-T016, T052-T066)

**Parallel Opportunities**: 17 tasks marked [P]

**MVP Scope**: Phase 1 + Phase 2 + Phase 3 (User Story 1) = 28 tasks
