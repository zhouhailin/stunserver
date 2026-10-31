# Feature Specification: STUN RFC Error Handling & Attribute Interoperability

**Feature Branch**: `007-stun-rfc-error-handling`

**Created**: 2026-10-31

**Status**: Draft

**Input**: User description: "按 P0 开始修复，做 1+2+5（错误响应、属性码、真实向量测试）。client 基于 RFC 增加响应的测试场景。"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Server rejects unknown comprehension-required attributes with 420 (Priority: P1)

A STUN client uses an extension attribute (for example PRIORITY `0x0024` from ICE, or a vendor-private attribute below `0x8000`) that this server does not implement. Today the server silently returns a success response, so the client wrongly concludes the extension is supported and may make incorrect NAT traversal decisions. The server must instead return a Binding Error Response with error code 420 (Unknown Attribute) that names exactly the attributes it did not understand.

**Why this priority**: This is the single highest-impact RFC 5389 compliance gap. Section 7.3.1 makes the 420 response mandatory, and returning success for unsupported request attributes actively misleads clients.

**Independent Test**: Send a Binding Request over UDP containing one unknown comprehension-required attribute and confirm the reply is a Binding Error Response carrying ERROR-CODE 420 and an UNKNOWN-ATTRIBUTES list containing that attribute type.

**Acceptance Scenarios**:

1. **Given** a running server, **When** it receives a Binding Request with a single unknown comprehension-required attribute (e.g. `0x0024`), **Then** it replies with message type `0x0111` (Binding Error Response), the request's transaction ID and magic cookie, an ERROR-CODE attribute with class 4 and number 20, and an UNKNOWN-ATTRIBUTES attribute listing `0x0024`.
2. **Given** a request containing two unknown comprehension-required attributes, **When** the server processes it, **Then** both type codes appear in UNKNOWN-ATTRIBUTES, each listed once, and the ERROR-CODE is still 420.
3. **Given** a request containing only unknown comprehension-optional attributes (type `0x8000` or higher, e.g. `0x80FF`), **When** the server processes it, **Then** it replies with a normal Binding Success Response (`0x0101`) and does not list those attributes anywhere.
4. **Given** a request with a valid known attribute (e.g. SOFTWARE), **When** the server processes it, **Then** behavior is unchanged: a success response with XOR-MAPPED-ADDRESS is returned.
5. **Given** an error response is produced, **When** the client inspects it, **Then** it contains no XOR-MAPPED-ADDRESS and the ERROR-CODE reason phrase is a human-readable string such as "Unknown Attribute".
6. **Given** an error response is logged at DEBUG level, **When** the summary is read, **Then** it shows the error code number and the UNKNOWN-ATTRIBUTES type list so the rejection can be diagnosed.

---

### User Story 2 - Attribute registry uses correct RFC 5780 codepoints (Priority: P1)

A client performing NAT behavior discovery sends CHANGE-REQUEST (`0x0003`), PADDING (`0x0026`), or RESPONSE-PORT (`0x0027`), and interprets RESPONSE-ORIGIN (`0x802B`) / OTHER-ADDRESS (`0x802C`) in responses. The current registry mislabels `0x802B` as SOURCE-ADDRESS, `0x802C` as CHANGED-ADDRESS, and `0x802D` as OTHER-ADDRESS, so these attributes are logged and classified under the wrong names and requests carrying them are silently accepted. Because this deployment has no alternate IP/port, CHANGE-REQUEST cannot be honored and must be rejected per RFC 5780 section 6.

**Why this priority**: Wrong codepoints undermine every log line, summary, and future RFC 5780 feature. Correcting them is a prerequisite for safely classifying request attributes in User Story 1.

**Independent Test**: Encode and decode each attribute type and confirm it is named per RFC 5389/RFC 5780; then send a CHANGE-REQUEST request and confirm it is answered with 420 rather than success.

**Acceptance Scenarios**:

1. **Given** the protocol attribute registry, **When** it resolves types `0x0003`, `0x0026`, and `0x0027`, **Then** they are reported as CHANGE-REQUEST, PADDING, and RESPONSE-PORT respectively.
2. **Given** the registry, **When** it resolves types `0x802B`, `0x802C`, and `0x8027`, **Then** they are reported as RESPONSE-ORIGIN, OTHER-ADDRESS, and CACHE-TIMEOUT respectively; `0x802D` is no longer treated as OTHER-ADDRESS.
3. **Given** a single-address server, **When** it receives a Binding Request with CHANGE-REQUEST, **Then** it replies with a 420 error response and lists CHANGE-REQUEST in UNKNOWN-ATTRIBUTES (RFC 5780 section 6).
4. **Given** the server does not implement padding or response-port redirection, **When** it receives PADDING or RESPONSE-PORT, **Then** it replies with 420 rather than silently succeeding.
5. **Given** any success response, **When** inspected, **Then** it never contains OTHER-ADDRESS (no alternate address is configured) and the server does not advertise NAT-behavior-discovery service names.

---

### User Story 3 - Wire format validated against real RFC 5769 test vectors (Priority: P2)

A maintainer refactors message encoding/decoding. The current "RFC 5769" tests only round-trip self-generated messages, so an encoder/decoder pair that is wrong in the same way still passes. The test suite must consume the exact hex byte sequences published in RFC 5769 sections 2.1 through 2.4 and assert both decoding results and byte-for-byte re-encoding.

**Why this priority**: Real vectors are the only way to prove interoperability with third-party STUN implementations. They also pin the behavior that User Stories 1 and 2 rely on.

**Independent Test**: Run the protocol test suite offline; each published vector decodes to the documented message type, transaction ID, attribute sequence and mapped address, and re-encodes to the identical byte string.

**Acceptance Scenarios**:

1. **Given** the RFC 5769 section 2.1 request bytes, **When** decoded, **Then** it is a Binding Request with transaction ID `b7e7a701bc34d686fa87dfae`, attributes SOFTWARE, PRIORITY, ICE-CONTROLLED, USERNAME, MESSAGE-INTEGRITY and FINGERPRINT in order, and FINGERPRINT/MESSAGE-INTEGRITY values are preserved byte-for-byte.
2. **Given** the section 2.2 IPv4 response bytes, **When** decoded, **Then** the XOR-MAPPED-ADDRESS resolves to `192.0.2.1:32853`.
3. **Given** the section 2.3 IPv6 response bytes, **When** decoded, **Then** the XOR-MAPPED-ADDRESS resolves to `[2001:db8:1234:5678:11:2233:4455:6677]:32853`.
4. **Given** the section 2.4 long-term authentication request bytes, **When** decoded, **Then** USERNAME (UTF-8), NONCE, REALM, MESSAGE-INTEGRITY and FINGERPRINT are recognized with their documented lengths.
5. **Given** any of the four decoded vectors, **When** re-encoded, **Then** the result is byte-for-byte identical to the published vector.

---

### User Story 4 - Client consumes RFC-defined responses (Priority: P2)

An operator uses the bundled client to diagnose NAT/firewall behavior. The client currently treats any non-success message as an opaque "unexpected message type" and assumes every success response is trustworthy. It must instead interpret RFC-defined error responses (showing the error code and the unknown attributes), tolerate unknown comprehension-optional attributes, and reject success responses that contain unknown comprehension-required attributes.

**Why this priority**: Without this, the 420 responses added in User Story 1 are unusable by the project's own tooling, and the client cannot detect a misbehaving server.

**Independent Test**: Exercise the client's response parsing with crafted successes and errors covering IPv4/IPv6, 420, unknown optional and required attributes, transaction mismatch and malformed packets; each yields the documented outcome.

**Acceptance Scenarios**:

1. **Given** a Binding Success Response with an IPv4 XOR-MAPPED-ADDRESS, **When** parsed, **Then** the client reports the mapped address and latency.
2. **Given** a Binding Success Response with an IPv6 XOR-MAPPED-ADDRESS, **When** parsed, **Then** the client reports the IPv6 mapped address.
3. **Given** a Binding Error Response (`0x0111`) with ERROR-CODE 420 and UNKNOWN-ATTRIBUTES, **When** parsed, **Then** the client returns a structured protocol error exposing the error class, number, reason phrase, and the list of unknown attribute types.
4. **Given** a success response containing an unknown comprehension-optional attribute, **When** parsed, **Then** the unknown attribute is ignored and the mapped address is still returned.
5. **Given** a success response containing an unknown comprehension-required attribute, **When** parsed, **Then** the client rejects it with a protocol error naming the offending attribute.
6. **Given** a response with a mismatched transaction ID, a truncated header, or a malformed attribute length, **When** parsed, **Then** the client rejects it with a protocol error.

---

### Edge Cases

- Unknown comprehension-required attribute appears twice in one request: UNKNOWN-ATTRIBUTES lists the type once.
- Both unknown comprehension-required and comprehension-optional attributes are present: only the required-range types appear in UNKNOWN-ATTRIBUTES.
- ERROR-CODE with a reason phrase that is not a multiple of four bytes: it is padded on the wire but the parsed reason phrase excludes padding.
- UNKNOWN-ATTRIBUTES attribute is absent from a 420 response: the client reports the error with an empty list rather than failing to parse.
- A request containing both PADDING and RESPONSE-PORT: RFC 5780 requires a 400 error; when both are unsupported this specification accepts a 420 response as the documented behavior (see Assumptions).
- Message length in header does not match actual bytes or is not a multiple of four: decoding fails cleanly.
- XOR-MAPPED-ADDRESS family byte indicates IPv6 but attribute length is IPv4-sized (or vice versa): decoding fails cleanly.
- A response arrives from a different source address than the server being queried: existing client behavior (connection/protocol error) is preserved.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The server MUST reply with a Binding Error Response (message type `0x0111`) when a request contains one or more unknown comprehension-required attributes (type range `0x0000`–`0x7FFF`).
- **FR-002**: A 420 response MUST include an ERROR-CODE attribute whose class is 4 and number is 20, with a non-empty UTF-8 reason phrase.
- **FR-003**: A 420 response MUST include UNKNOWN-ATTRIBUTES listing every unrecognized comprehension-required attribute type as 16-bit values, de-duplicated, padded to a four-byte boundary.
- **FR-004**: Error responses MUST echo the request's 96-bit transaction ID and magic cookie, and MUST NOT include XOR-MAPPED-ADDRESS.
- **FR-005**: Unknown comprehension-optional attributes (`0x8000`–`0xFFFF`) MUST NOT trigger an error and MUST NOT be echoed in UNKNOWN-ATTRIBUTES.
- **FR-006**: The attribute registry MUST recognize `0x0003` CHANGE-REQUEST, `0x0026` PADDING, `0x0027` RESPONSE-PORT, `0x8027` CACHE-TIMEOUT, `0x802B` RESPONSE-ORIGIN, and `0x802C` OTHER-ADDRESS with their RFC 5780 names and MUST stop labeling `0x802B`/`0x802C` as SOURCE-ADDRESS/CHANGED-ADDRESS and `0x802D` as OTHER-ADDRESS.
- **FR-007**: Because the deployment has a single address and port, a request carrying CHANGE-REQUEST MUST be answered with 420 and CHANGE-REQUEST listed in UNKNOWN-ATTRIBUTES, per RFC 5780 section 6.
- **FR-008**: Requests carrying PADDING or RESPONSE-PORT MUST be answered with 420; the server MUST NOT silently accept them.
- **FR-009**: Success responses MUST NOT contain OTHER-ADDRESS, and the server MUST NOT advertise the `stun-behavior`/`stun-behaviors` service names while it lacks an alternate address.
- **FR-010**: The test suite MUST decode the exact byte sequences from RFC 5769 sections 2.1, 2.2, 2.3 and 2.4 and assert message type, transaction ID, attribute types/order/lengths, and decoded mapped addresses.
- **FR-011**: Re-encoding each decoded RFC 5769 vector MUST produce a byte-for-byte identical message.
- **FR-012**: MESSAGE-INTEGRITY and FINGERPRINT MUST be preserved and length-validated, but cryptographic verification is out of scope for this feature.
- **FR-013**: The client MUST distinguish Binding Success Responses from Binding Error Responses and, for errors, expose the error class, number, reason phrase, and unknown-attribute list as a structured protocol error.
- **FR-014**: The client MUST ignore unknown comprehension-optional attributes in success responses, and MUST reject success responses containing unknown comprehension-required attributes.
- **FR-015**: The client MUST continue to reject transaction ID mismatches, malformed headers, and malformed attribute lengths, and MUST reject XOR-MAPPED-ADDRESS whose address family is unsupported or whose length does not match the family.
- **FR-016**: Message summaries and DEBUG logs MUST render ERROR-CODE numbers and UNKNOWN-ATTRIBUTES type lists so error responses are diagnosable, and MUST use the corrected RFC 5780 names.

### Key Entities *(include if feature involves data)*

- **Attribute Registry Entry**: A 16-bit type code paired with its RFC name and comprehension class (required vs optional); drives validation and logging.
- **Error Response**: A STUN message of type `0x0111` carrying an ERROR-CODE and, for 420, an UNKNOWN-ATTRIBUTES list, bound to the request's transaction ID.
- **Unknown-Attributes List**: An ordered, de-duplicated set of 16-bit attribute type codes, padded to four bytes.
- **Test Vector Fixture**: A published RFC 5769 message expressed as exact bytes, with its documented transaction ID, attribute sequence, and expected decoded values.
- **Client Response Outcome**: The result of parsing a response: mapped address plus latency, or a structured protocol error carrying error details.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of requests carrying at least one unknown comprehension-required attribute receive a 420 error response that lists exactly those types, verified by end-to-end UDP tests.
- **SC-002**: 0 attribute types in the registry resolve to a name that contradicts RFC 5389 or RFC 5780.
- **SC-003**: All four published RFC 5769 vectors (sections 2.1–2.4) decode to the documented values and re-encode byte-for-byte identically.
- **SC-004**: The client response test suite covers at least 8 distinct scenarios including IPv4 success, IPv6 success, 420 error with UNKNOWN-ATTRIBUTES, unknown optional attribute, unknown required attribute, transaction mismatch, and malformed input, with all passing.
- **SC-005**: The full workspace test suite, formatting check, and lint check pass with no warnings before the feature is considered complete.

## Assumptions

- The server continues to operate with a single public address and port; adding an alternate address/port is out of scope, so CHANGE-REQUEST remains unsupported and yields 420.
- PADDING and RESPONSE-PORT support (RFC 5780 sections 7.5 and 7.6) is out of scope; unsupported comprehension-required attributes are rejected with 420. In the combined PADDING + RESPONSE-PORT case, 420 is accepted in place of the RFC 5780-mandated 400.
- SOURCE-ADDRESS (`0x0004`) and CHANGED-ADDRESS (`0x0005`) from legacy RFC 3489 are not required; the registry will not expose them under RFC 5780 codes.
- MESSAGE-INTEGRITY and FINGERPRINT are handled as opaque, length-checked attributes; HMAC/CRC verification and credential handling are out of scope. Because MESSAGE-INTEGRITY is comprehension-required and unverifiable here, a request carrying it receives 420 instead of the silent discard implied by RFC 5389 section 10.1.2 (see research.md).
- The bundled client remains a simple binding client with no credentials; new behavior is limited to interpreting responses and reporting errors.
- Existing success-response behavior (XOR-MAPPED-ADDRESS only) is retained; adding MAPPED-ADDRESS or RESPONSE-ORIGIN is out of scope for this P0 batch.
- Failing tests are written before implementation per the project constitution's TDD requirement.
