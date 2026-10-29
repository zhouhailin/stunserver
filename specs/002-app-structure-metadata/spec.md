# Feature Specification: Application Structure and Metadata Enhancement

**Feature Branch**: `002-app-structure-metadata`

**Created**: 2026-10-29

**Status**: Draft

**Input**: User description: "功能点优化。代码目录结果优化，统一归到 crates 和 src 目录下，在 src/server 对应 stunserver src/client 对应 stunclient src/bench 对应 stunbench 。 应用启动时输出一行日志，该日志包含应用名称、应用版本、commit id 时间，编译时间、启动时间，时间输出格式统一，时区与服务器时区一致。stunclient 发起请求，如果没有携带端口，则使用3478端口，支持域名解析，测试地址 stunserver2025.stunprotocol.org。三个应用 --help也需要输出应用版本相关信息"

## Clarifications

### Session 2026-10-29

- Q: What format should the startup log line use? → A: Plain text with fixed positions (e.g., "stunserver v0.1.0 commit=abc123 compiled=2026-10-29T10:00:00+08:00 started=2026-10-29T10:05:00+08:00")
- Q: When should the startup log line be output relative to other initialization operations? → A: After configuration and argument parsing, but before network binding or main event loop start
- Q: Should the startup log line be suppressible via a command-line flag or environment variable? → A: Yes, via a --quiet or --no-startup-log command-line flag
- Q: When DNS resolution returns multiple IP addresses, which address should stunclient use? → A: Try addresses in order returned by DNS, falling back to the next if connection fails
- Q: Where should version information appear in the --help output? → A: At the top of help output as the first line
- Q: Which RFC documents in docs/rfcs/ are mandatory for implementation compliance versus reference-only? → A: RFC 5389 and 5769 are mandatory; RFC 5780 is optional/extension; RFC 3489 is reference-only (obsoleted)
- Q: Should the full commit ID be printed in logs and help output? → A: No, use short commit ID (first 7-8 characters) for readability
- Q: What precision should timestamps use? → A: Second precision without fractional seconds (format: YYYY-MM-DDTHH:MM:SS±HH:MM)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Reorganized Project Structure (Priority: P1)

As a developer working on the STUN server project, I want the codebase to be organized with application code in `src/` subdirectories (server, client, bench) and shared libraries in `crates/`, so that the project structure follows Rust workspace best practices and makes it easier to locate and maintain code.

**Why this priority**: This is foundational infrastructure work that affects all subsequent development. A clean, standard project structure reduces cognitive load for new contributors and aligns with Rust ecosystem conventions.

**Independent Test**: Can be fully tested by verifying the directory structure matches the target layout (`src/server/`, `src/client/`, `src/bench/`, `crates/`), all existing tests pass, and all three binaries build successfully. Delivers immediate value by improving code navigability.

**Acceptance Scenarios**:

1. **Given** the current project structure with separate top-level directories for each binary, **When** the restructuring is complete, **Then** application code resides in `src/server/`, `src/client/`, and `src/bench/` directories, shared libraries are in `crates/`, and the project builds without errors.

2. **Given** the restructured project, **When** running `cargo build --release`, **Then** three binaries are produced: `stunserver`, `stunclient`, and `stunbench`.

3. **Given** the restructured project, **When** running `cargo test`, **Then** all existing unit and integration tests pass with no regressions.

4. **Given** a developer navigating the codebase, **When** they want to modify server logic, **Then** they can find the server code in `src/server/` and shared protocol code in `crates/` following standard Rust workspace conventions.

---

### User Story 2 - Startup Metadata Logging (Priority: P2)

As an operator deploying the STUN server, I want each application to output a single structured log line on startup containing the application name, version, commit ID, compilation timestamp, and startup timestamp, so that I can easily identify which version is running and when it was deployed for troubleshooting and audit purposes.

**Why this priority**: Operational visibility is critical for production deployments. Having version and build metadata readily available in logs simplifies debugging, rollback decisions, and compliance auditing. This is high-value but can be implemented after the structural reorganization.

**Independent Test**: Can be fully tested by starting each application (stunserver, stunclient, stunbench) and verifying the startup log line contains all required fields in the correct format. Delivers immediate operational value.

**Acceptance Scenarios**:

1. **Given** the stunserver binary is built and deployed, **When** it starts up, **Then** it outputs exactly one log line to stdout containing: application name ("stunserver"), version string, commit ID, compilation timestamp, and startup timestamp.

2. **Given** the stunclient binary is executed, **When** it starts, **Then** it outputs exactly one log line with the same metadata format as stunserver, using "stunclient" as the application name.

3. **Given** the stunbench binary is executed, **When** it starts, **Then** it outputs exactly one log line with the same metadata format, using "stunbench" as the application name.

4. **Given** any of the three applications outputs a startup log, **When** examining the timestamps, **Then** all timestamps use the same format (ISO 8601 or similar) and reflect the server's local timezone, not UTC.

5. **Given** the application is built with a specific git commit, **When** the startup log is generated, **Then** the short commit ID (first 7-8 characters) matches the corresponding prefix of `git rev-parse HEAD` output at build time.

---

### User Story 3 - Enhanced Client Default Port and DNS Resolution (Priority: P2)

As a user of the stunclient tool, I want the client to default to port 3478 when no port is specified and to support domain name resolution, so that I can run simple tests without remembering the standard STUN port and can test against domain names like stunserver2025.stunprotocol.org.

**Why this priority**: This improves usability and aligns with STUN protocol conventions (port 3478 is the standard). DNS resolution support is essential for real-world testing against production STUN servers. Equal priority to startup logging as both are user-facing improvements.

**Independent Test**: Can be fully tested by running stunclient with various address formats (IP only, IP:port, domain, domain:port) and verifying correct behavior. Delivers immediate usability improvement.

**Acceptance Scenarios**:

1. **Given** a user runs stunclient with only an IP address (e.g., `stunclient 192.168.1.1`), **When** the client initiates the STUN binding request, **Then** it connects to port 3478 by default.

2. **Given** a user runs stunclient with an IP and explicit port (e.g., `stunclient 192.168.1.1:3479`), **When** the client initiates the request, **Then** it connects to the specified port 3479, not the default.

3. **Given** a user runs stunclient with a domain name (e.g., `stunclient stunserver2025.stunprotocol.org`), **When** the client initiates the request, **Then** it resolves the domain to an IP address and connects to port 3478.

4. **Given** a user runs stunclient with a domain and port (e.g., `stunclient stunserver2025.stunprotocol.org:3479`), **When** the client initiates the request, **Then** it resolves the domain and connects to the specified port 3479.

5. **Given** the domain name cannot be resolved, **When** the user runs stunclient with that domain, **Then** the client outputs a clear error message indicating DNS resolution failure and exits with a non-zero status code.

6. **Given** the test address stunserver2025.stunprotocol.org is reachable, **When** a user runs `stunclient stunserver2025.stunprotocol.org`, **Then** the client successfully completes the STUN binding transaction and displays the result.

---

### User Story 4 - Version Information in Help Output (Priority: P3)

As a user of the STUN tools, I want the `--help` output for each application (stunserver, stunclient, stunbench) to display the application version and build information, so that I can quickly verify which version I'm running without starting the application.

**Why this priority**: This is a nice-to-have improvement for user experience and debugging. While useful, it's lower priority than the structural and operational improvements. Can be implemented last.

**Independent Test**: Can be fully tested by running each application with `--help` or `-h` flag and verifying version information appears in the output. Delivers incremental usability improvement.

**Acceptance Scenarios**:

1. **Given** the stunserver binary, **When** a user runs `stunserver --help`, **Then** the first line of output displays the application name, version string, short commit ID (first 7-8 characters), and compilation timestamp in the format "stunserver {version} (commit {short_commit_id}, compiled {compile_time})".

2. **Given** the stunclient binary, **When** a user runs `stunclient --help`, **Then** the output includes the application name, version string, and build metadata.

3. **Given** the stunbench binary, **When** a user runs `stunbench --help`, **Then** the output includes the application name, version string, and build metadata.

4. **Given** any application's help output, **When** the user examines the version information, **Then** the format is consistent across all three applications and matches the format used in the startup log.

5. **Given** a user runs `stunserver -h` (short form), **When** the help is displayed, **Then** it includes the same version information as `--help`.

---

### Edge Cases

- What happens when the application is built from a dirty git working tree (uncommitted changes)? The commit ID should still be captured, and ideally the version string indicates uncommitted changes (e.g., "0.1.0-dirty" or similar).

- What happens when the build environment doesn't have git available? The compilation should still succeed, with commit ID set to "unknown" or omitted gracefully.

- What happens when the server's timezone configuration changes between builds? The startup log should reflect the current server timezone at runtime, not the timezone at build time.

- What happens when stunclient is given an IPv6 address? The client should handle IPv6 addresses correctly, with proper syntax (e.g., `[::1]:3478`).

- What happens when DNS resolution returns multiple IP addresses? The client MUST try addresses in the order returned by DNS, falling back to the next address if connection fails, and report which IP was successfully used in the output.

- What happens when the default port 3478 is already in use by another process? The application should fail to bind and output a clear error message suggesting the user specify a different port.

- What happens when the startup log is redirected to a file? The log should be written to stdout so it can be captured by log aggregation systems or redirected as needed.

- What happens when the application version is not set in Cargo.toml? The build should use a default version (e.g., "0.0.0-unknown") rather than failing.

- What happens when the user passes the --quiet flag? The startup log line should be suppressed, and no output should appear on stdout during initialization.

## Requirements *(mandatory)*

### Functional Requirements

**Directory Structure and Build System**

- **FR-001**: The project MUST reorganize application code into `src/server/`, `src/client/`, and `src/bench/` directories, with each directory containing the main entry point and application-specific logic for the respective binary.

- **FR-002**: Shared libraries and protocol implementations MUST be located in the `crates/` directory as separate Cargo workspace members.

- **FR-003**: The Cargo workspace configuration MUST be updated to reflect the new directory structure, with all three binaries building successfully from the reorganized codebase.

- **FR-004**: All existing unit tests and integration tests MUST pass after the restructuring without modification to test logic (only path updates allowed).

- **FR-005**: The project MUST maintain backward compatibility in terms of binary names and command-line interfaces; users should not need to change how they invoke the applications.

**Startup Metadata Logging**

- **FR-006**: Each application (stunserver, stunclient, stunbench) MUST output exactly one log line to stdout upon startup, after configuration and argument parsing but before network binding or main event loop start. The log line MUST use plain text format with fixed positions in the pattern: "{app_name} v{version} commit={short_commit_id} compiled={compile_time} started={startup_time}". The commit ID MUST be truncated to the first 7-8 characters for readability.

- **FR-007**: The startup log line MUST include the following fields: application name, application version, git commit ID (short form, first 7-8 characters), compilation timestamp, and startup timestamp.

- **FR-008**: All timestamps in the startup log MUST use a consistent format (ISO 8601 without fractional seconds: YYYY-MM-DDTHH:MM:SS±HH:MM) and reflect the server's local timezone at the time of logging.

- **FR-009**: The git commit ID MUST be captured at compile time using build scripts or environment variables, reflecting the exact commit from which the binary was built. For display purposes, the commit ID MUST be truncated to the first 7-8 characters (short form).

- **FR-010**: The compilation timestamp MUST be captured at build time and embedded in the binary, not computed at runtime.

- **FR-011**: If the binary is built from a dirty working tree (uncommitted changes), the version string SHOULD indicate this status (e.g., append "-dirty" suffix).

- **FR-012**: If git is not available during the build, the commit ID field SHOULD be set to "unknown" rather than causing the build to fail.

- **FR-012a**: All three applications MUST support a `--quiet` or `--no-startup-log` command-line flag that suppresses the startup log line output. When this flag is present, no startup log should be written to stdout.

**Client Default Port and DNS Resolution**

- **FR-013**: The stunclient application MUST default to port 3478 when the user provides an address without an explicit port number.

- **FR-014**: The stunclient application MUST support domain name resolution, allowing users to specify hostnames (e.g., "stunserver2025.stunprotocol.org") in addition to IP addresses.

- **FR-015**: When a domain name is provided, stunclient MUST resolve it to an IP address before initiating the STUN binding request.

- **FR-016**: If DNS resolution fails, stunclient MUST output a clear error message indicating the failure and exit with a non-zero status code.

- **FR-017**: The stunclient application MUST support both IPv4 and IPv6 addresses and domain names.

- **FR-018**: The address parsing logic MUST handle the following formats: "IP", "IP:port", "domain", "domain:port", "[IPv6]:port".

- **FR-018a**: When DNS resolution returns multiple IP addresses for a domain, stunclient MUST attempt to connect to each address in the order returned by the DNS resolver, falling back to the next address if a connection attempt fails, until a successful connection is established or all addresses have been tried.

**Version Information in Help Output**

- **FR-019**: All three applications (stunserver, stunclient, stunbench) MUST display version information when invoked with `--help` or `-h` flags.

- **FR-020**: The version information in help output MUST include: application name, version string, git commit ID (short form, first 7-8 characters), and compilation timestamp.

- **FR-021**: The format of version information in help output MUST be consistent across all three applications.

- **FR-022**: The version information MUST appear as the first line of the --help output, followed by the standard help content (usage, options, etc.). Format: "{app_name} {version} (commit {short_commit_id}, compiled {compile_time})". The commit ID MUST be truncated to the first 7-8 characters.

**RFC Compliance**

- **FR-023**: All STUN protocol implementations (stunserver, stunclient, stunbench) MUST strictly conform to RFC 5389 (Session Traversal Utilities for NAT) and RFC 5769 (Test Vectors for STUN). Compliance with these RFCs is mandatory and non-negotiable.

- **FR-024**: Implementation SHOULD support RFC 5780 (NAT Behavior Discovery Using STUN) as an optional extension. When implemented, it MUST conform to the RFC 5780 specification.

- **FR-025**: RFC 3489 (original STUN specification) is obsoleted by RFC 5389 and serves as historical reference only. Implementation MUST NOT include behaviors that conflict with RFC 5389 in favor of RFC 3489 compatibility.

- **FR-026**: The project MUST maintain a local copy of relevant RFC documents in `docs/rfcs/` directory for developer reference. Implementation code SHOULD include comments or documentation referencing specific RFC sections where protocol behavior is defined.

### Key Entities

- **Application Metadata**: A collection of build-time and runtime information including application name, version, commit ID (stored as short form, first 7-8 characters for display), compilation timestamp, and startup timestamp. This metadata is immutable after compilation (except for startup timestamp which is captured at runtime).

- **Network Address**: A parsed representation of a target server address, including hostname or IP, port number, and address family (IPv4/IPv6). The address may be provided in various string formats and must be normalized before use.

- **Startup Log Entry**: A single-line, structured log message output by each application at startup. Contains all application metadata fields in a consistent, machine-parseable format.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All three binaries (stunserver, stunclient, stunbench) build successfully from the restructured codebase with zero compilation errors and zero clippy warnings.

- **SC-002**: 100% of existing unit and integration tests pass after the directory restructuring, with no test logic modifications required (only path updates permitted).

- **SC-003**: Each application outputs the startup log line after configuration parsing and before network operations, within 100 milliseconds of completing argument parsing.

- **SC-004**: The startup log line contains all five required fields (app name, version, short commit ID, compile time, startup time) in plain text format with fixed positions, and can be parsed by standard log aggregation tools (e.g., grep, awk, sed). The commit ID is truncated to 7-8 characters for readability.

- **SC-005**: The stunclient successfully connects to stunserver2025.stunprotocol.org:3478 using only the domain name (no explicit port), demonstrating both DNS resolution and default port functionality.

- **SC-006**: The stunclient correctly handles at least five different address formats (IP, IP:port, domain, domain:port, IPv6) without user-facing errors or crashes.

- **SC-007**: Running `--help` on any of the three applications displays version information that matches the information in the startup log (same version string, short commit ID, and compilation timestamp).

- **SC-008**: The project structure follows Rust workspace best practices, with clear separation between application code (src/) and shared libraries (crates/), making it intuitive for new contributors to navigate.

- **SC-009**: Build time increases by no more than 10% after restructuring (due to workspace reorganization), measured as the time to complete `cargo build --release` from a clean state.

- **SC-010**: All timestamps in logs and help output use the server's local timezone and a consistent format, verified by comparing output across applications running on the same host.

- **SC-011**: All STUN protocol implementations pass validation against RFC 5769 test vectors, demonstrating strict conformance to RFC 5389 message formats, attribute types, and error codes.

- **SC-012**: Code review verification confirms that protocol-related code includes references to specific RFC 5389 and RFC 5769 sections, and no implementation conflicts with mandatory RFC requirements.

## Assumptions

- The current project uses a Cargo workspace structure with separate crates for each binary and shared protocol libraries. The restructuring will preserve this workspace approach while reorganizing directory locations.

- The project already has a build script mechanism (build.rs) or can add one to capture git commit ID and compilation timestamp at build time without significantly impacting build complexity.

- Users of the STUN tools are comfortable with command-line interfaces and will benefit from structured log output that can be parsed by tools like grep, awk, or log aggregation systems.

- The standard STUN port 3478 (as defined in RFC 5389) is the appropriate default for stunclient, and users expect this behavior based on STUN protocol conventions.

- DNS resolution will use the system's standard resolver (e.g., libc getaddrinfo or Rust's ToSocketAddrs trait), which respects /etc/hosts, DNS search domains, and other system-level configurations.

- The project's CI/CD pipeline (if any) will be updated separately to reflect the new directory structure. This specification covers only the source code reorganization, not CI configuration changes.

- Git is available in the build environment for most users. The fallback behavior (commit ID = "unknown") handles edge cases where git is not installed or the code is built from a source tarball without git metadata.

- The application version is defined in Cargo.toml using the standard `version` field, and this version is used consistently across all binaries and in help output.

- Timestamps will use the chrono crate or similar well-maintained Rust library for timezone-aware datetime formatting, ensuring consistent output across platforms.

- The restructuring will be performed in a single pull request to minimize merge conflicts and make the change easier to review. Incremental restructuring (multiple PRs) is possible but not recommended due to the tight coupling between directory structure and Cargo workspace configuration.

- RFC 5389 and RFC 5769 compliance is mandatory for all STUN protocol implementations. RFC 5780 support is optional and may be implemented as an extension. RFC 3489 is obsoleted and serves only as historical reference. Developers MUST consult the RFC documents in `docs/rfcs/` when implementing or modifying protocol-related code.

- The RFC documents in `docs/rfcs/` are authoritative references for protocol behavior. In case of ambiguity between this specification and the RFC documents, the RFC documents take precedence for protocol-level requirements.
