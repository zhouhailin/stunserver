# Feature Specification: Rust STUN Server Project Initialization

**Feature Branch**: `001-rust-stun-project-init`

**Created**: 2026-10-29

**Status**: Draft

**Input**: User description: "初始化工程及基础代码。该工程采用rust编程语言，单元测试覆盖率需要达到 95%+。该工程编译后需要包含stunserver、stunclient、stunbench可执行程序。stunserver 运行时支持指定配置文件，支持命令行直接运行和采用systemd运行两种方式。stunclient可以用于测试。stunbench用于服务端压力测试，运行时通过命令行指定qps。stunserver 运行采用异步编程方案，qps需要支持1万。代码提交push到github仓库，需要支持自动编译，打tag时支持自动编译各个运行平台版本，可以人工下载。上面三个可执行程序运行时需要输出，最近一次commit id，提交时间，编译时间，应用版本，启动时间，采用UTC时间，获取系统默认时区。"

## Clarifications

### Session 2026-10-29

- Q: What network transport protocol(s) should the STUN server support for receiving and responding to STUN messages? → A: UDP only - Standard STUN transport, lower latency, simpler implementation, matches typical use
- Q: What file format should be used for the stunserver configuration file? → A: YAML - Human-readable, widely used in DevOps (Kubernetes, Docker), flexible but indentation-sensitive
- Q: Should the STUN server and client support IPv4 only, IPv6 only, or both IPv4 and IPv6 address families? → A: Both IPv4 and IPv6 (dual-stack) - Maximum compatibility, modern standard, slightly more complex
- Q: What logging levels should the stunserver support for operational event logging? → A: Four levels: error, warn, info, debug - Standard practice, granular control
- Q: What are the code commenting requirements? → A: Detailed English comments must be added during code writing process
- Q: What are the logging output requirements for each executable? → A: stunserver supports both console and file logging with configurable path, size limit, date-based rotation, and max file count; stunclient and stunbench are console-only

## User Scenarios & Testing *(mandatory)*

### User Story 1 - STUN Server Deployment and Operation (Priority: P1)

A system administrator needs to deploy and run a high-performance STUN server to support NAT traversal for real-time communication applications. The administrator can start the server using command line with optional configuration file, or deploy it as a systemd service for production environments. Upon startup, the server displays build metadata including version, commit ID, build time, and startup time in both UTC and local timezone for operational visibility and troubleshooting.

**Why this priority**: This is the core deliverable - without a working STUN server, the project has no value. It must support both development (command line) and production (systemd) deployment modes.

**Independent Test**: Can be fully tested by starting the server with default configuration, verifying it accepts STUN binding requests, and confirming startup metadata is displayed correctly. Delivers immediate operational value.

**Acceptance Scenarios**:

1. **Given** the server binary is built, **When** executed without arguments, **Then** it starts with default configuration and displays version, commit ID, commit time, build time, and startup time (UTC and local timezone)
2. **Given** a configuration file exists, **When** the server is started with `--config <path>`, **Then** it loads and applies the specified configuration
3. **Given** the server is running as a systemd service, **When** the service is started via `systemctl start stunserver`, **Then** it runs as a background daemon and logs to systemd journal
4. **Given** the server is handling requests, **When** 10,000 requests per second are sent, **Then** it processes all requests without degradation

---

### User Story 2 - STUN Client Testing Tool (Priority: P2)

A developer or QA engineer needs to test STUN server functionality using a command-line client tool. The client can send STUN binding requests to a specified server and display the response including the mapped address and port. This enables manual testing, integration testing, and debugging of STUN server deployments.

**Why this priority**: Testing capability is essential for validating server functionality and debugging issues. It's the primary way to verify the server works correctly.

**Independent Test**: Can be fully tested by running the client against a running server instance and verifying it receives valid STUN responses. Delivers testing and validation capability.

**Acceptance Scenarios**:

1. **Given** a STUN server is running, **When** the client sends a binding request to the server address, **Then** it displays the server's response including mapped IP address and port
2. **Given** the client is executed, **When** it runs, **Then** it displays version, commit ID, commit time, build time, and startup time (UTC and local timezone)
3. **Given** the server is unreachable, **When** the client attempts to connect, **Then** it displays a clear error message with timeout information

---

### User Story 3 - STUN Benchmark Tool (Priority: P2)

A performance engineer needs to validate that the STUN server can handle the required load of 10,000 queries per second. The benchmark tool sends STUN requests at a specified QPS rate and reports success rate, latency percentiles, and errors. This enables capacity planning and performance validation before production deployment.

**Why this priority**: Performance validation is critical for production readiness. The server must demonstrably handle the required load.

**Independent Test**: Can be fully tested by running the benchmark tool with a specified QPS against a running server and verifying it reports metrics. Delivers performance validation capability.

**Acceptance Scenarios**:

1. **Given** a STUN server is running, **When** the benchmark tool is executed with `--qps 10000`, **Then** it sends requests at 10,000 per second and reports success rate, latency percentiles (p50, p95, p99), and error count
2. **Given** the benchmark tool is executed, **When** it runs, **Then** it displays version, commit ID, commit time, build time, and startup time (UTC and local timezone)
3. **Given** the server cannot handle the specified QPS, **When** the benchmark runs for 60 seconds, **Then** it reports the actual achieved QPS and error rate

---

### User Story 4 - Automated Build and Release Pipeline (Priority: P3)

A release engineer needs automated builds to ensure code quality and produce release artifacts. Every push to the repository triggers a build and test cycle. When a version tag is created, the system automatically builds binaries for all target platforms (Linux, macOS, Windows) and makes them available for manual download. This ensures consistent, reproducible builds and simplifies release distribution.

**Why this priority**: Automation reduces manual effort and ensures quality gates are consistently applied. Cross-platform builds enable broader deployment.

**Independent Test**: Can be fully tested by pushing code and verifying the build runs, then creating a tag and verifying platform-specific binaries are produced and downloadable. Delivers automated quality assurance and release distribution.

**Acceptance Scenarios**:

1. **Given** code is pushed to the repository, **When** the push occurs, **Then** an automated build runs, executes all tests, and reports pass/fail status
2. **Given** a version tag (e.g., v1.0.0) is created, **When** the tag is pushed, **Then** the system builds binaries for Linux (x86_64, aarch64), macOS (x86_64, aarch64), and Windows (x86_64)
3. **Given** platform binaries are built, **When** a user accesses the release page, **Then** they can download the appropriate binary for their platform
4. **Given** tests fail during automated build, **When** the build completes, **Then** the failure is reported and no release artifacts are produced

---

### User Story 5 - Build Metadata and Version Tracking (Priority: P3)

An operations engineer needs to identify exactly which version of the software is running in production for troubleshooting and audit purposes. Each executable displays build metadata at startup including the git commit ID, commit timestamp, build timestamp, application version, and startup time in both UTC and local timezone. This enables precise version tracking and correlation with source code.

**Why this priority**: Operational visibility is important for production support but doesn't affect core functionality. It's a quality-of-life feature for operations.

**Independent Test**: Can be fully tested by running any executable and verifying the metadata output format and accuracy. Delivers operational visibility.

**Acceptance Scenarios**:

1. **Given** any executable (stunserver, stunclient, stunbench) is started, **When** it initializes, **Then** it displays: application version, git commit ID (short hash), commit time (UTC), build time (UTC), startup time (UTC), and system timezone
2. **Given** the executable is built from a specific commit, **When** the commit ID is displayed, **Then** it matches the git commit from which it was built
3. **Given** the system timezone is configured, **When** startup time is displayed, **Then** it shows both UTC time and the equivalent local time based on system timezone

---

### Edge Cases

- What happens when the configuration file path does not exist or is unreadable? The server should fail to start with a clear error message indicating the file path and reason.
- What happens when the server cannot bind to the configured port (e.g., port already in use)? The server should fail to start with a clear error message indicating the port and suggesting alternatives.
- What happens when the benchmark tool specifies a QPS that exceeds the server's capacity? The benchmark should report the actual achieved QPS, error rate, and latency degradation.
- What happens when the client cannot reach the server (network unreachable or timeout)? The client should report a clear timeout error with the attempted address and duration.
- What happens when the git repository has uncommitted changes during build? The build should still succeed but the commit ID should reflect the last committed state, and a warning should indicate uncommitted changes exist.
- What happens when building on a platform without cross-compilation toolchain? The build should fail with a clear message indicating the missing toolchain and how to install it.

## Requirements *(mandatory)*

### Functional Requirements

**Core Executables**

- **FR-001**: System MUST produce three separate executable programs: stunserver, stunclient, and stunbench
- **FR-002**: Each executable MUST display build metadata at startup including: application version, git commit ID, commit timestamp, build timestamp, startup time (UTC), and system timezone
- **FR-003**: All timestamps MUST be displayed in both UTC and local timezone (derived from system timezone configuration)

**STUN Server**

- **FR-004**: stunserver MUST accept STUN Binding Request messages over UDP and return valid STUN Binding Response messages per RFC 5389, supporting both IPv4 and IPv6 address families (dual-stack)
- **FR-005**: stunserver MUST support loading configuration from a file specified via command-line argument
- **FR-006**: stunserver MUST support running as a foreground process (command-line execution) for development and testing
- **FR-007**: stunserver MUST support running as a systemd service for production deployment
- **FR-008**: stunserver MUST use asynchronous I/O for network operations to achieve high concurrency
- **FR-009**: stunserver MUST handle a minimum of 10,000 queries per second under normal operating conditions
- **FR-010**: stunserver MUST support logging to both console (stdout/stderr) and file output with configurable logging levels: error, warn, info, debug
- **FR-010a**: stunserver log file configuration MUST support: file path, maximum file size, date-based rotation, and maximum number of files to retain
- **FR-010b**: stunclient MUST output logs to console only (stdout/stderr), no file logging
- **FR-010c**: stunbench MUST output logs to console only (stdout/stderr), no file logging

**STUN Client**

- **FR-011**: stunclient MUST send STUN Binding Request messages over UDP to a specified server address, supporting both IPv4 and IPv6 addresses
- **FR-012**: stunclient MUST display the STUN Binding Response including mapped IP address and port
- **FR-013**: stunclient MUST support specifying target server address via command-line argument
- **FR-014**: stunclient MUST report clear error messages when server is unreachable or times out

**STUN Benchmark Tool**

- **FR-015**: stunbench MUST send STUN Binding Requests over UDP at a specified queries-per-second (QPS) rate, supporting both IPv4 and IPv6 target addresses
- **FR-016**: stunbench MUST accept target QPS as a command-line argument
- **FR-017**: stunbench MUST report success rate, error count, and latency percentiles (p50, p95, p99) after test completion
- **FR-018**: stunbench MUST support specifying test duration or number of requests via command-line argument
- **FR-019**: stunbench MUST display real-time progress during test execution

**Build and Release**

- **FR-020**: System MUST include automated build pipeline triggered on every push to the repository
- **FR-021**: Automated build MUST compile all three executables and run all tests
- **FR-022**: When a version tag is created, system MUST build binaries for multiple target platforms (Linux x86_64, Linux aarch64, macOS x86_64, macOS aarch64, Windows x86_64)
- **FR-023**: Release artifacts MUST be made available for manual download from the repository release page
- **FR-024**: Build metadata (commit ID, build time) MUST be embedded in executables at compile time

**Testing and Quality**

- **FR-025**: Unit test coverage MUST be at least 95% of codebase
- **FR-026**: All code MUST pass formatting checks (rustfmt)
- **FR-027**: All code MUST pass linting checks (clippy) with zero warnings
- **FR-028**: Integration tests MUST verify end-to-end functionality of server-client interaction
- **FR-029**: All source code MUST include detailed English comments explaining functionality, logic, and design decisions

### Key Entities

- **STUN Message**: Protocol message with type (Request/Response/Error), transaction ID, and attributes. Follows RFC 5389 format.
- **Configuration**: Server settings in YAML format including listen address, port, logging level (error, warn, info, debug), log output mode (console/file/both), log file path, maximum log file size, log rotation strategy (date-based), and maximum number of log files to retain. Loaded from file or command-line defaults.
- **Build Metadata**: Immutable information embedded at compile time including version, commit ID, commit time, and build time.
- **Benchmark Result**: Test output including achieved QPS, success rate, latency percentiles, and error statistics.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: stunserver successfully processes 10,000 STUN binding requests per second with less than 1% error rate
- **SC-002**: stunclient receives valid STUN binding response within 100 milliseconds when server is on local network
- **SC-003**: stunbench accurately measures and reports server performance metrics with less than 5% measurement error
- **SC-004**: All three executables display complete and accurate build metadata (version, commit ID, timestamps) at startup
- **SC-005**: Automated build pipeline completes within 10 minutes for standard push events
- **SC-006**: Release build produces downloadable binaries for all five target platforms within 30 minutes of tag creation
- **SC-007**: Unit test coverage remains at or above 95% throughout development
- **SC-008**: System administrator can deploy stunserver as systemd service and start/stop/restart it using standard systemctl commands
- **SC-009**: Developer can run stunclient and receive valid STUN response from running server within 5 seconds of execution
- **SC-010**: Performance engineer can validate server handles required 10,000 QPS load using stunbench before production deployment

## Assumptions

- Target users have basic familiarity with command-line tools and systemd service management
- Production deployment environments run Linux with systemd (version 219 or later)
- Development environments have Rust toolchain (stable, latest edition) installed
- Git repository is hosted on GitHub with GitHub Actions enabled for CI/CD
- Network connectivity between client and server is stable with less than 1% packet loss for testing scenarios
- Server hardware meets minimum requirements: 2 CPU cores, 2 GB RAM, gigabit network interface for 10,000 QPS operation
- Configuration file format uses YAML (YAML Ain't Markup Language) for human-readable, DevOps-friendly configuration
- Version numbering follows semantic versioning (MAJOR.MINOR.PATCH) convention
- Release tags follow format "v{MAJOR}.{MINOR}.{PATCH}" (e.g., v1.0.0, v1.2.3)
- Build metadata embedding uses compile-time mechanisms (build scripts, environment variables) to capture git information
- System timezone is correctly configured on the host operating system via standard system configuration
- STUN protocol implementation strictly follows RFC 5389 without extensions (TURN, ICE) in initial version
- STUN messages are transported exclusively over UDP (User Datagram Protocol); TCP is not supported
- Server and client support dual-stack networking (both IPv4 and IPv6 address families)
- Logging output goes to stdout/stderr for command-line execution and systemd journal for service execution
- Benchmark tool measures client-side metrics; server-side metrics are out of scope for stunbench
