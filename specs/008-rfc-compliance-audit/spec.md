# Feature Specification: STUN RFC 5389/5769/5780 Conformance Audit and Remediation

**Feature Branch**: `008-rfc-compliance-audit`

**Created**: 2026-10-31

**Status**: Draft

**Input**: User description: "参考 rfc5389.txt rfc5769.txt rfc5780.txt 帮我检查代码实现"

## Background and Audit Summary

The maintainer asked for the current implementation to be checked against the three reference RFCs in `docs/rfcs/`: RFC 5389 (STUN), RFC 5769 (STUN test vectors), and RFC 5780 (NAT behavior discovery attributes). This specification converts the audit into a priority-ordered, independently testable plan whose first deliverable is a traceable conformance matrix, followed by remediation of the gaps found.

### Confirmed conformant (no behavior change requested)

- The 20-byte header layout, magic cookie `0x2112A442`, network byte order, 4-byte attribute alignment, and transaction-ID echo.
- MAPPED-ADDRESS and XOR-MAPPED-ADDRESS IPv4/IPv6 encode/decode, including the RFC 5769 §2.2 and §2.3 expected addresses.
- 420 (Unknown Attribute) responses with ERROR-CODE and UNKNOWN-ATTRIBUTES for unknown comprehension-required attributes, including de-duplication and padding.
- RFC 5780 attribute codepoints and rejection of CHANGE-REQUEST, PADDING, and RESPONSE-PORT by a single-address server.
- Byte-identical decode/re-encode of all four RFC 5769 §2.1–2.4 vectors.

### Gaps found by the audit

1. **Incomplete attribute registry and naming.** The RFC 5389 attributes USERNAME (`0x0006`), MESSAGE-INTEGRITY (`0x0008`), REALM (`0x0014`), NONCE (`0x0015`), ALTERNATE-SERVER (`0x8023`), and FINGERPRINT (`0x8028`) are not recognized by name, so diagnostics and summaries show only raw type codes.
2. **Known attributes mistaken for unknown, and no authentication support.** Every comprehension-required request attribute is treated as unknown and rejected with 420, including USERNAME and MESSAGE-INTEGRITY. The clarified direction is to add RFC 5389 §10 authentication (both short-term and long-term credentials): USERNAME and MESSAGE-INTEGRITY become actively validated against configured credentials, other known-but-unexpected attributes are ignored per RFC 5389 §7.3, and only genuinely unsupported attributes produce 420.
3. **FINGERPRINT never validated.** A request carrying an incorrect FINGERPRINT, a FINGERPRINT that is not the last attribute, or a duplicate FINGERPRINT is still processed. RFC 5389 §7.3 and §15.5 require validation and silent discard on failure.
4. **MESSAGE-INTEGRITY ordering not honored.** Attributes after MESSAGE-INTEGRITY are still classified and can trigger 420, although RFC 5389 §15.4 says all attributes after MESSAGE-INTEGRITY except FINGERPRINT MUST be ignored.
5. **Missing SOFTWARE attribute.** Responses do not identify the software, although RFC 5389 §7.3.1.1 says the server SHOULD add a SOFTWARE attribute.
6. **Client transaction IDs are time-derived.** The client's 96-bit transaction IDs are built from the system clock and are not uniformly random, contrary to RFC 5389 §6 (MUST be uniformly random; SHOULD be cryptographically random).
7. **RFC 5769 integrity values unverified.** The four published vectors are decoded and re-encoded but their HMAC-SHA1 MESSAGE-INTEGRITY and CRC-32 FINGERPRINT values are never checked, so wire-format integrity is unproven.
8. **Client hardening gaps.** The client does not send SOFTWARE (RFC 5389 §7.1 SHOULD) and does not validate FINGERPRINT in responses.
9. **RFC 5780 non-claims are not fully regression-tested.** The server must not advertise NAT behavior discovery, yet no test asserts the absence of OTHER-ADDRESS/RESPONSE-ORIGIN or the absence of `stun-behavior` service advertising.

## Clarifications

### Session 2026-10-31

- Q: When a Binding Request carries RFC 5389 attributes the server does not act on (USERNAME, MESSAGE-INTEGRITY, REALM, NONCE, MAPPED-ADDRESS, ERROR-CODE, UNKNOWN-ATTRIBUTES), should the server ignore them and return success, or keep rejecting with 420? → A: Support authentication: validate MESSAGE-INTEGRITY against configured credentials and include MESSAGE-INTEGRITY in responses.
- Q: Which RFC 5389 §10 authentication mechanism should the server support? → A: Both short-term and long-term credentials (full RFC 5389 §10).
- Q: How should the server decide whether a Binding Request is authenticated with short-term credentials, long-term credentials, or not authenticated at all? → A: Configuration selects exactly one mode (disabled, short-term, or long-term); in an enabled mode every Binding Request must authenticate, and requests without credential attributes are rejected.
- Q: Should the bundled diagnostic client also gain authentication support in this feature? → A: Yes — full client authentication: short-term plus long-term 401/438 challenge handling with NONCE retry, and credential flags.
- Q: For long-term credential key derivation, must passwords be normalized with the full SASLprep algorithm (RFC 4013), or may the implementation support only ASCII passwords and document the limitation? → A: Full SASLprep (RFC 4013) normalization for long-term passwords; non-ASCII passwords interoperate as specified.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Traceable conformance audit matrix (Priority: P1)

A maintainer needs a clause-by-clause record of how the implementation satisfies RFC 5389, RFC 5769, and RFC 5780, so that compliance statements are evidence-based and future changes cannot silently regress conformance.

**Why this priority**: This is the direct deliverable of the requested check. It defines the audit scope, records current gaps, and gives every later fix a traceable home.

**Independent Test**: Review the matrix and spot-check at least ten entries against the RFC text and the named test evidence; every in-scope clause has a status and every gap maps to a requirement in this specification.

**Acceptance Scenarios**:

1. **Given** the conformance matrix, **When** it is reviewed, **Then** every clause in scope (RFC 5389 §6, §7.1–§7.3, §15; RFC 5769 §2; RFC 5780 §6–§7 for a server without NAT behavior discovery) has a status of conformant, gap, or out-of-scope.
2. **Given** a clause marked as a gap, **When** the matrix is read, **Then** it names the requirement in this specification that closes it.
3. **Given** a clause marked out-of-scope, **When** the matrix is read, **Then** it states why (for example, RFC 3489 backward compatibility, TCP/TLS transport, or NAT behavior discovery is not offered).
4. **Given** both MUST and SHOULD clauses, **When** they are recorded, **Then** each carries its RFC normative level and the test or manual evidence that demonstrates it.
5. **Given** a future protocol behavior change, **When** the change is reviewed, **Then** the matrix is updated in the same change.

---

### User Story 2 - Known versus unknown attributes are distinguished correctly (Priority: P1)

A client sends a Binding Request that includes RFC 5389 attributes such as USERNAME or MESSAGE-INTEGRITY. The server currently rejects the whole request with 420, treating standard attributes as unknown. The server must ignore known-but-unexpected attributes, reject only genuinely unsupported comprehension-required attributes, and name every known attribute in its diagnostics.

**Why this priority**: Returning 420 for standard RFC 5389 attributes breaks interoperability with legitimate clients and contradicts RFC 5389 §7.3.

**Independent Test**: Send crafted Binding Requests containing only known attributes, only unknown required attributes, and mixtures of both; verify success versus 420 and the exact UNKNOWN-ATTRIBUTES contents.

**Acceptance Scenarios**:

1. **Given** a request containing only known RFC 5389 attributes that do not change Binding processing and are not authentication attributes (for example MAPPED-ADDRESS, ERROR-CODE, UNKNOWN-ATTRIBUTES, ALTERNATE-SERVER), **When** the server processes it, **Then** it returns a Binding Success Response rather than 420.
2. **Given** a request containing an unknown comprehension-required attribute such as PRIORITY (`0x0024`), **When** the server processes it, **Then** it returns 420 and lists exactly that type in UNKNOWN-ATTRIBUTES.
3. **Given** a request mixing known attributes and one or more unknown comprehension-required attributes, **When** the server processes it, **Then** only the unknown types appear in UNKNOWN-ATTRIBUTES.
4. **Given** a request containing unknown comprehension-optional attributes, **When** the server processes it, **Then** it returns success and never lists those types.
5. **Given** a request containing CHANGE-REQUEST, PADDING, or RESPONSE-PORT on this single-address server, **When** the server processes it, **Then** it still returns 420 as before.
6. **Given** any request, **When** it is summarized at DEBUG level, **Then** every RFC 5389 and RFC 5780 attribute type is shown by its RFC name and unrecognized types are shown as `0xNNNN`.

---

### User Story 3 - Authenticated Binding requests (Priority: P1)

A client configured with credentials includes USERNAME and MESSAGE-INTEGRITY in its Binding Request, using either short-term or long-term credential semantics. The server must validate the integrity using the configured credential before processing the request and must protect its response with MESSAGE-INTEGRITY. Requests with missing, unknown, stale, or invalid credentials must receive the RFC-defined authentication error rather than a success response.

**Why this priority**: Authentication was selected explicitly for this feature. Without it, MESSAGE-INTEGRITY behavior cannot be correct, the server cannot serve authenticated usages, and the RFC 5769 integrity vectors cannot be exercised end to end.

**Independent Test**: Send requests with valid, unknown, missing, stale, and tampered credentials under both mechanisms and inspect the response type, error code, and presence or absence of MESSAGE-INTEGRITY, USERNAME, REALM, and NONCE.

**Acceptance Scenarios**:

1. **Given** short-term authentication is required and a request carries a valid USERNAME and matching MESSAGE-INTEGRITY, **When** the server processes it, **Then** it returns a success response containing a MESSAGE-INTEGRITY computed with the same credential and no USERNAME.
2. **Given** short-term authentication is required and a request omits USERNAME or MESSAGE-INTEGRITY, **When** the server processes it, **Then** it returns 400 and the response contains neither MESSAGE-INTEGRITY nor USERNAME.
3. **Given** short-term authentication is required and the USERNAME is unknown or the MESSAGE-INTEGRITY value is incorrect, **When** the server processes it, **Then** it returns 401 and the response contains neither MESSAGE-INTEGRITY nor USERNAME.
4. **Given** long-term authentication is required and a request omits MESSAGE-INTEGRITY, **When** the server processes it, **Then** it returns 401 containing REALM and NONCE, and containing neither MESSAGE-INTEGRITY nor USERNAME.
5. **Given** long-term authentication is required and MESSAGE-INTEGRITY is present but USERNAME, REALM, or NONCE is missing, **When** the server processes it, **Then** it returns 400 and the response contains neither MESSAGE-INTEGRITY nor USERNAME.
6. **Given** long-term authentication is required and the request's NONCE is stale or expired, **When** the server processes it, **Then** it returns 438 containing a fresh NONCE and REALM, and containing neither MESSAGE-INTEGRITY nor USERNAME.
7. **Given** long-term authentication is required and the USERNAME is unknown, the REALM does not match, or the MESSAGE-INTEGRITY value is incorrect, **When** the server processes it, **Then** it returns 401 containing REALM and NONCE, and containing neither MESSAGE-INTEGRITY nor USERNAME.
8. **Given** a long-term authenticated request with valid credentials, **When** the server returns success, **Then** the response contains MESSAGE-INTEGRITY and does not contain USERNAME, REALM, or NONCE.
9. **Given** an authenticated request that also contains an unknown comprehension-required attribute, **When** the server processes it, **Then** it returns 420 containing MESSAGE-INTEGRITY in addition to ERROR-CODE and UNKNOWN-ATTRIBUTES.
10. **Given** authentication is enabled in either mode and a request carries no credential attributes, **When** the server processes it, **Then** it is rejected as unauthenticated (400 short-term; 401 with REALM and NONCE long-term) and never processed as a public Binding request.
11. **Given** any authentication failure, **When** the request is summarized or logged, **Then** no password or other credential secret appears in the output.

---

### User Story 4 - Message intake enforces FINGERPRINT and MESSAGE-INTEGRITY rules (Priority: P1)

A client or intermediary includes FINGERPRINT and MESSAGE-INTEGRITY attributes. The server must validate and honor their ordering rules, silently discarding damaged or misordered messages and ignoring attributes that RFC 5389 says must be ignored.

**Why this priority**: These are explicit RFC 5389 §7.3, §15.4, and §15.5 MUST-level behaviors; accepting a message with a bad FINGERPRINT undermines protocol integrity checks and can let malformed traffic trigger error responses.

**Independent Test**: Send requests with valid, invalid, duplicated, and misordered FINGERPRINT/MESSAGE-INTEGRITY combinations and confirm response versus silent discard for each case.

**Acceptance Scenarios**:

1. **Given** a request whose FINGERPRINT is the last attribute and whose CRC is correct, **When** the server processes it, **Then** it returns the appropriate success or 420 response.
2. **Given** a request whose FINGERPRINT CRC is incorrect, **When** the server processes it, **Then** it sends no response and discards the message.
3. **Given** a request where FINGERPRINT is not the last attribute (including a duplicate FINGERPRINT), **When** the server processes it, **Then** it sends no response.
4. **Given** a request where a comprehension-required attribute follows MESSAGE-INTEGRITY, **When** the server classifies attributes, **Then** that attribute is ignored and does not cause 420.
5. **Given** a request with MESSAGE-INTEGRITY followed by a valid FINGERPRINT, **When** the server processes it, **Then** FINGERPRINT is still validated as the final attribute and the request succeeds if no unknown required attribute is present and authentication succeeds (or authentication is disabled).
6. **Given** a request with a header defect (non-zero leading bits, wrong magic cookie, length not a multiple of four, length beyond the datagram), an unsupported method, or a class other than Binding Request, **When** the server receives it, **Then** it sends no response.

---

### User Story 5 - Responses are complete and do not overclaim RFC 5780 support (Priority: P2)

An operator inspects server responses with a packet capture. Every response identifies the server software, success responses carry the client's reflexive address, error responses carry a usable error code, and no response claims NAT behavior discovery features the server does not provide.

**Why this priority**: SOFTWARE is an RFC 5389 SHOULD that materially helps interoperability debugging; avoiding OTHER-ADDRESS/RESPONSE-ORIGIN prevents clients from attempting unsupported NAT behavior tests.

**Independent Test**: Capture success and 420 responses and inspect their attribute sets, then confirm the documented deployment does not advertise the `stun-behavior` service names.

**Acceptance Scenarios**:

1. **Given** a successful Binding Request, **When** the response is inspected, **Then** it contains XOR-MAPPED-ADDRESS reflecting the source address and SOFTWARE, and echoes the transaction ID.
2. **Given** a request that triggers 420, **When** the response is inspected, **Then** it contains ERROR-CODE (class 4, number 20), UNKNOWN-ATTRIBUTES, and SOFTWARE, and no XOR-MAPPED-ADDRESS.
3. **Given** any response, **When** its SOFTWARE value is inspected, **Then** it is valid UTF-8 shorter than 128 characters and unchanged across repeated requests.
4. **Given** any response, **When** its attribute set is inspected, **Then** OTHER-ADDRESS and RESPONSE-ORIGIN are absent.
5. **Given** the project documentation and deployment configuration, **When** reviewed, **Then** they do not advertise `stun-behavior` or `stun-behaviors` DNS SRV names.

---

### User Story 6 - RFC 5769 vectors prove integrity correctness (Priority: P2)

A protocol maintainer needs the published RFC 5769 vectors to prove not only that fields decode correctly, but also that the implementation computes and validates MESSAGE-INTEGRITY and FINGERPRINT exactly as specified.

**Why this priority**: The existing tests prove structural decode/re-encode only. Integrity verification is the part most likely to be wrong in a subtle way, and it anchors the intake rules in User Story 3.

**Independent Test**: Run the protocol test suite offline; each published vector must reproduce the documented HMAC-SHA1 and CRC-32 values and re-encode byte-for-byte.

**Acceptance Scenarios**:

1. **Given** the RFC 5769 §2.1 request bytes and published password, **When** MESSAGE-INTEGRITY is verified, **Then** it matches the published 20 bytes and FINGERPRINT matches the published CRC value.
2. **Given** the RFC 5769 §2.2 IPv4 response, **When** MESSAGE-INTEGRITY and FINGERPRINT are verified, **Then** both match the published values.
3. **Given** the RFC 5769 §2.3 IPv6 response, **When** MESSAGE-INTEGRITY and FINGERPRINT are verified, **Then** both match the published values.
4. **Given** the RFC 5769 §2.4 long-term credential request, **When** the key is derived per RFC 5389 §15.4 from the documented username, realm, and password, **Then** MESSAGE-INTEGRITY matches the published value.
5. **Given** any byte covered by MESSAGE-INTEGRITY or FINGERPRINT is altered, **When** verification runs, **Then** it fails.
6. **Given** any of the four vectors, **When** decoded and re-encoded, **Then** the result remains byte-for-byte identical.

---

### User Story 7 - Client follows the same protocol rules (Priority: P3)

An operator uses the bundled client to diagnose connectivity, including against an authenticated server. The client must use unpredictable transaction IDs, identify itself, reject responses whose FINGERPRINT does not validate, and support both short-term and long-term authentication with nonce challenge handling, so its results are trustworthy.

**Why this priority**: The client is a diagnostic tool; its non-compliance does not affect server interoperability, but it should not model incorrect protocol behavior and must be able to exercise the server's new authentication paths.

**Independent Test**: Exercise the client's ID generation, request contents, response validation, and both authenticated flows (direct short-term request and long-term 401/438 challenge retry) with crafted valid and invalid responses.

**Acceptance Scenarios**:

1. **Given** 10,000 newly generated transaction IDs, **When** collected, **Then** all are 12 bytes, all are distinct, and they are not the repeating clock-derived pattern used today.
2. **Given** a Binding Request sent by the client, **When** captured, **Then** it contains a SOFTWARE attribute.
3. **Given** a response with a valid terminal FINGERPRINT, **When** parsed, **Then** the client accepts it; given an invalid or non-terminal FINGERPRINT, **Then** the client reports a protocol error.
4. **Given** a response with an unknown comprehension-required attribute, **When** parsed, **Then** the client rejects it; given an unknown comprehension-optional attribute, **Then** the client ignores it.
5. **Given** a Binding Error Response, **When** parsed, **Then** the client exposes the error class, number, reason, and unknown-attribute list.
6. **Given** a response from an address other than the queried server or with a mismatched transaction ID, **When** parsed, **Then** the client rejects it.
7. **Given** the client is configured with a short-term shared key, **When** it sends a Binding Request, **Then** the request contains USERNAME and a MESSAGE-INTEGRITY computed with that key.
8. **Given** the client is configured with long-term credentials and receives a 401 carrying REALM and NONCE, **When** it retries, **Then** the retry contains USERNAME, REALM, NONCE, and a MESSAGE-INTEGRITY derived from the long-term key, and a successful response's MESSAGE-INTEGRITY is validated.
9. **Given** a long-term retry receives 438 carrying a fresh NONCE, **When** the client retries again, **Then** it uses the fresh NONCE within a bounded number of attempts before reporting a structured error.
10. **Given** the client is not configured with credentials and receives 401 or 438, **When** it processes the response, **Then** it reports the structured error without retrying, and no credential secret appears in its output.

---

### Edge Cases

- The same unknown comprehension-required attribute appears twice: it is listed once.
- A request contains both known and unknown comprehension-required attributes: only the unknown types are rejected and listed.
- An unknown comprehension-required attribute follows MESSAGE-INTEGRITY: it is ignored and does not cause 420.
- A request has a valid FINGERPRINT plus an unknown comprehension-required attribute: FINGERPRINT validation passes, then 420 is returned.
- A request has an invalid FINGERPRINT plus an unknown comprehension-required attribute: the message is silently discarded; no 420 is sent.
- Authentication is checked before unknown-attribute classification: a request that fails authentication receives 400, 401, or 438 even when it also contains unknown comprehension-required attributes.
- Short-term authentication: a request missing USERNAME or MESSAGE-INTEGRITY receives 400; an unknown USERNAME or a MESSAGE-INTEGRITY mismatch receives 401.
- Long-term authentication: a request missing MESSAGE-INTEGRITY receives 401 with REALM and NONCE; MESSAGE-INTEGRITY present but USERNAME, REALM, or NONCE missing receives 400; a stale NONCE receives 438 with a fresh NONCE and REALM; an unknown USERNAME, a REALM mismatch, or a MESSAGE-INTEGRITY mismatch receives 401 with REALM and NONCE.
- A long-term password contains non-ASCII characters that SASLprep maps, normalizes, or removes: key derivation uses the normalized form, and a client supplying an equivalent password authenticates successfully.
- An authenticated request that passes integrity validation but contains an unknown comprehension-required attribute receives 420 carrying MESSAGE-INTEGRITY.
- Authentication is enabled in a given mode and a request carries no credential attributes: it is rejected and never receives an unauthenticated success.
- Authentication is disabled and a request carries USERNAME, MESSAGE-INTEGRITY, REALM, or NONCE: those attributes are ignored and no error is generated.
- The client is configured with a short-term key: it authenticates every request directly; a 401 or 438 is reported as a terminal error without retry.
- The client is configured for long-term credentials: it sends an initial unauthenticated request, retries after a 401 or 438 challenge with the supplied REALM and NONCE, and treats repeated challenges beyond the retry bound as a terminal error.
- A response to an authenticated client request lacks MESSAGE-INTEGRITY or carries one that does not validate: the client rejects it.
- A FINGERPRINT appears twice, or any attribute follows the final FINGERPRINT: the message is discarded.
- A FINGERPRINT has a length other than 4 bytes, or MESSAGE-INTEGRITY a length other than 20 bytes: the message is discarded.
- The header's leading two bits are non-zero, the magic cookie is wrong, the length is not a multiple of four, or the length exceeds the received datagram: no response.
- The message is a Binding Indication, a success response, an error response, or an unsupported method: no response.
- Attributes are truncated or padding is missing: no response.
- IPv4 and IPv6 source addresses both produce a correctly XOR-obfuscated mapped address.
- A retransmitted Binding Request with the same transaction ID and contents produces an equivalent response without retaining per-transaction state.

## Requirements *(mandatory)*

### Functional Requirements

**Attribute recognition and classification**

- **FR-001**: The attribute registry MUST recognize, by name, RFC 5389 types MAPPED-ADDRESS (`0x0001`), USERNAME (`0x0006`), MESSAGE-INTEGRITY (`0x0008`), ERROR-CODE (`0x0009`), UNKNOWN-ATTRIBUTES (`0x000A`), REALM (`0x0014`), NONCE (`0x0015`), XOR-MAPPED-ADDRESS (`0x0020`), SOFTWARE (`0x8022`), ALTERNATE-SERVER (`0x8023`), and FINGERPRINT (`0x8028`), in addition to the RFC 5780 types already recognized (CHANGE-REQUEST `0x0003`, PADDING `0x0026`, RESPONSE-PORT `0x0027`, CACHE-TIMEOUT `0x8027`, RESPONSE-ORIGIN `0x802B`, OTHER-ADDRESS `0x802C`).
- **FR-002**: Message and attribute summaries MUST render every recognized type by its RFC name and every unrecognized type as a four-digit hexadecimal code.
- **FR-003**: Comprehension-required types MUST be those in the range `0x0000`–`0x7FFF`; comprehension-optional types MUST be those in `0x8000`–`0xFFFF`.
- **FR-004**: The server MUST return 420 only when at least one comprehension-required request attribute is unrecognized or unsupported, and MUST list exactly those types, de-duplicated in first-seen order, in UNKNOWN-ATTRIBUTES.
- **FR-005**: The server MUST ignore known-but-unexpected comprehension-required RFC 5389 attributes that are not authentication or fingerprint attributes and are not RFC 5780-mandated rejections (for example MAPPED-ADDRESS, ERROR-CODE, UNKNOWN-ATTRIBUTES, ALTERNATE-SERVER) rather than rejecting them with 420; USERNAME, MESSAGE-INTEGRITY, REALM, and NONCE are handled by the authentication requirements.
- **FR-006**: On this single-address server, requests carrying CHANGE-REQUEST, PADDING, or RESPONSE-PORT MUST still be answered with 420, preserving RFC 5780 §6 behavior.
- **FR-007**: Unknown comprehension-optional attributes MUST be ignored and MUST NOT appear in UNKNOWN-ATTRIBUTES.

**Message intake validation**

- **FR-008**: Before attribute processing, the server MUST validate the header: the two most significant bits are zero, the magic cookie is `0x2112A442`, the message length is a multiple of four and does not exceed the received datagram, the method is Binding, and the class is allowed; any failure MUST cause silent discard with no response.
- **FR-009**: The server MUST silently discard messages that are not Binding Requests, including indications, success responses, error responses, and unsupported methods.
- **FR-010**: When FINGERPRINT is present, the server MUST require it to be the final attribute, exactly 4 bytes long, and equal to the CRC-32 of the preceding message bytes XOR `0x5354554E`; any violation MUST cause silent discard with no response.
- **FR-011**: MESSAGE-INTEGRITY, when present, MUST be exactly 20 value bytes; a different length MUST cause silent discard.
- **FR-012**: The server MUST ignore every attribute that follows MESSAGE-INTEGRITY except FINGERPRINT for all classification and processing decisions.
- **FR-013**: Malformed or truncated attributes, including missing padding, MUST cause silent discard with no response.

**Response formation**

- **FR-014**: A Binding Success Response MUST contain XOR-MAPPED-ADDRESS derived from the request's source transport address, and MUST echo the request's transaction ID and magic cookie.
- **FR-015**: A 420 response MUST contain ERROR-CODE with class 4 and number 20 and a human-readable reason phrase, and UNKNOWN-ATTRIBUTES; it MUST NOT contain XOR-MAPPED-ADDRESS.
- **FR-016**: Every success and error response MUST include SOFTWARE whose value is valid UTF-8, shorter than 128 characters, and identifies the server software and version.
- **FR-017**: The server MUST NOT include OTHER-ADDRESS or RESPONSE-ORIGIN in any response, and project documentation and deployment configuration MUST NOT advertise the `stun-behavior` or `stun-behaviors` service names.
- **FR-018**: A UDP response MUST be sent to the request's source address from the same local address and port on which the request was received.

**Authentication**

- **FR-019**: The server MUST support both the short-term and long-term credential mechanisms defined in RFC 5389 §10. Configuration MUST select exactly one authentication mode: disabled, short-term, or long-term. When a mode is enabled, every Binding Request MUST authenticate; there is no per-request fallback to unauthenticated processing.
- **FR-020**: Authentication MUST be evaluated after header and FINGERPRINT intake validation but before unknown-attribute classification and success response formation.
- **FR-021**: Under short-term credentials, a request missing USERNAME or MESSAGE-INTEGRITY MUST be rejected with 400.
- **FR-022**: Under short-term credentials, an unknown USERNAME or a MESSAGE-INTEGRITY that does not match the configured credential MUST be rejected with 401.
- **FR-023**: Under long-term credentials, a request missing MESSAGE-INTEGRITY MUST be rejected with 401 containing REALM and NONCE.
- **FR-024**: Under long-term credentials, a request carrying MESSAGE-INTEGRITY but missing USERNAME, REALM, or NONCE MUST be rejected with 400.
- **FR-025**: Under long-term credentials, a stale or expired NONCE MUST be rejected with 438 containing a fresh NONCE and the REALM.
- **FR-026**: Under long-term credentials, an unknown USERNAME, a REALM mismatch, or a MESSAGE-INTEGRITY that does not match the derived key MUST be rejected with 401 containing REALM and NONCE.
- **FR-027**: Short-term keys MUST use the configured key (or password) as defined in RFC 5389 §15.4; long-term keys MUST be derived as MD5 over `username ":" realm ":" SASLprep(password)`, where SASLprep MUST implement the full RFC 4013 algorithm (Unicode 3.2), including mapping, normalization, prohibited-output, and bidirectional-text checks.
- **FR-028**: A response to an authenticated Binding Request MUST include MESSAGE-INTEGRITY computed with the same credential; a success response MUST NOT include USERNAME and, under long-term credentials, MUST NOT include REALM or NONCE.
- **FR-029**: Authentication failure responses (400, 401, 438) MUST NOT include MESSAGE-INTEGRITY or USERNAME; long-term 401 and 438 responses MUST include REALM and NONCE.
- **FR-030**: A 420 response to an authenticated request MUST include MESSAGE-INTEGRITY in addition to ERROR-CODE and UNKNOWN-ATTRIBUTES.
- **FR-031**: Authentication MUST be configurable, including whether it is enforced, which mechanism is active, which credentials are valid, the realm, and the nonce lifetime.
- **FR-032**: NONCE values MUST be unpredictable, server-generated, and bound to a configurable lifetime; a NONCE the server cannot validate or that has expired MUST be treated as stale.
- **FR-033**: Credential secrets MUST NOT appear in logs, message summaries, or error text.

**Client behavior**

- **FR-034**: The client MUST choose a fresh 96-bit transaction ID uniformly at random for each new transaction, and SHOULD obtain it from a cryptographically secure source.
- **FR-035**: The client SHOULD include a SOFTWARE attribute in every Binding Request.
- **FR-036**: When a response contains FINGERPRINT, the client MUST require it to be the final attribute, exactly 4 bytes, and CRC-correct; otherwise it MUST report a protocol error.
- **FR-037**: The client MUST reject success responses containing unknown comprehension-required attributes, ignore unknown comprehension-optional attributes, reject transaction-ID mismatches, reject responses from unexpected source addresses, and expose ERROR-CODE class, number, reason, and UNKNOWN-ATTRIBUTES as structured errors.
- **FR-038**: The client MUST treat an ERROR-CODE with a class outside 3–6 or a number above 99 as a malformed response.
- **FR-039**: The client MUST support an explicit credential mode: disabled (default), short-term (username plus shared key), and long-term (username plus password); it MUST expose credential configuration to the operator and MUST NOT log credential secrets.
- **FR-040**: In short-term mode, every Binding Request MUST carry USERNAME and MESSAGE-INTEGRITY computed with the shared key; a 401 or 438 is reported as a terminal error without long-term retry.
- **FR-041**: In long-term mode, the client MUST send an initial unauthenticated request and, on a 401 or 438 carrying REALM and NONCE, retry with USERNAME, REALM, NONCE, and MESSAGE-INTEGRITY derived per RFC 5389 §15.4; it MUST adopt a fresh NONCE from a 438 and MUST bound the number of challenge retries.
- **FR-042**: When the client sends MESSAGE-INTEGRITY, it MUST validate the response's MESSAGE-INTEGRITY with the same credential and reject the response on failure; when no credentials are configured, a 401 or 438 MUST be reported as a structured error without retry.

**Verification and documentation**

- **FR-043**: The test suite MUST decode the exact RFC 5769 §2.1, §2.2, §2.3, and §2.4 byte sequences, verify their MESSAGE-INTEGRITY and FINGERPRINT values against the published credentials and values, and confirm byte-for-byte re-encoding.
- **FR-044**: Automated tests MUST cover every functional requirement, including negative cases for invalid or non-terminal FINGERPRINT, attributes after MESSAGE-INTEGRITY, known versus unknown required attributes, valid and invalid authentication under both mechanisms (including 400, 401, and 438 flows), SASLprep key-derivation cases including non-ASCII passwords, client short-term and long-term flows, header defects, and RFC 5780 rejections.
- **FR-045**: A conformance matrix MUST be maintained in project documentation for every in-scope MUST and SHOULD clause, recording normative level, status, evidence, and out-of-scope justification.

### Key Entities

- **STUN Message**: A 20-byte header (type, length, magic cookie, transaction ID) plus an ordered list of attributes; classified by method and class.
- **Attribute Definition**: A 16-bit type code with its RFC name, comprehension level, and this implementation's support status (supported, recognized-but-ignored, rejected).
- **Binding Transaction**: A client request and its response, correlated by the 96-bit transaction ID and the client's transport address.
- **Conformance Clause**: An RFC section, its normative level, the implementation status, and the test or manual evidence that demonstrates it.
- **Test Vector**: An exact byte sequence published in RFC 5769 together with its credentials and expected decoded values, MESSAGE-INTEGRITY, and FINGERPRINT.
- **Credential**: A configured username and shared key used to compute and verify MESSAGE-INTEGRITY; under long-term credentials the key is derived from the SASLprep-normalized password, username, and realm.
- **Realm**: The long-term credential scope supplied in challenges and used in key derivation.
- **Nonce**: An unpredictable, server-generated, time-limited value issued in long-term credential challenges and echoed by the client to prove freshness.
- **Authenticated Transaction**: A Binding transaction whose request and response are protected by MESSAGE-INTEGRITY computed with the same credential, using either the short-term or long-term mechanism.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All four RFC 5769 vectors decode to their documented values, pass MESSAGE-INTEGRITY and FINGERPRINT verification, and re-encode byte-for-byte identically (4 of 4).
- **SC-002**: 100% of in-scope MUST and SHOULD clauses in the conformance matrix are marked conformant with evidence or explicitly justified as out of scope; none remain unclassified.
- **SC-003**: In the automated suite, 100% of requests containing only known non-authentication RFC 5389 attributes receive a success response and 0 receive 420; when authentication is disabled, requests carrying authentication attributes also receive success.
- **SC-004**: In the negative test suite, 100% of messages with an invalid, duplicated, or non-terminal FINGERPRINT, or with a defective header, produce zero responses.
- **SC-005**: 100% of success and error responses carry SOFTWARE, and every 420 response lists exactly the unknown comprehension-required types present in the request.
- **SC-006**: Across 10,000 client-generated transaction IDs, all are 12 bytes and distinct, with no repetition of the current clock-derived pattern.
- **SC-007**: 0 responses contain OTHER-ADDRESS or RESPONSE-ORIGIN, 0 documentation or deployment artifacts advertise `stun-behavior`/`stun-behaviors`, and 100% of CHANGE-REQUEST tests return 420.
- **SC-008**: Enabling FINGERPRINT validation does not increase the p99 response latency of the existing request benchmark by more than 5%.
- **SC-009**: With authentication required, 100% of valid-credential requests under both short-term and long-term mechanisms receive a success response whose MESSAGE-INTEGRITY validates; 100% of missing-credential cases receive 400 (short-term missing attributes; long-term missing USERNAME/REALM/NONCE), 100% of missing-MESSAGE-INTEGRITY long-term cases and 100% of unknown-credential or tampered-MESSAGE-INTEGRITY cases receive 401, and 100% of stale-NONCE cases receive 438; 0 authentication failure responses contain MESSAGE-INTEGRITY or USERNAME; and 0 requests lacking credential attributes receive success while a mode is enabled.
- **SC-010**: Across the authentication test suite, 0 credential secrets appear in logs, message summaries, or error text.
- **SC-011**: 100% of short-term client requests authenticate and succeed against the server; 100% of long-term client flows complete the 401 challenge with a valid retry, adopt a fresh NONCE after 438, and validate the response's MESSAGE-INTEGRITY; 0 flows exceed the retry bound.
- **SC-012**: 100% of long-term key-derivation tests, including non-ASCII passwords that SASLprep maps, normalizes, or removes, match the RFC 4013 normalized form before MD5 derivation.

## Assumptions

- Authentication is in scope (see Clarifications): the server supports both the short-term and long-term credential mechanisms of RFC 5389 §10, validates MESSAGE-INTEGRITY using configured credentials, and protects authenticated responses with MESSAGE-INTEGRITY. Exactly one mode (disabled, short-term, or long-term) is selected by deployment configuration, together with the credentials, realm, and nonce lifetime. When authentication is disabled or unconfigured, the server behaves as a public Binding server and ignores authentication attributes.
- Short-term credentials assume the client knows the shared key out of band. Long-term credentials assume the server issues REALM and NONCE and that passwords are normalized with the full SASLprep algorithm (RFC 4013, Unicode 3.2) before key derivation, so non-ASCII credentials interoperate as specified; the MD5 key derivation is required even though MESSAGE-INTEGRITY itself remains HMAC-SHA1. Because the audited transport is plaintext UDP, the credential exchange and MESSAGE-INTEGRITY provide integrity, not confidentiality, and this limitation is documented.
- The client's credential mode is explicit and defaults to disabled; short-term uses a username and shared key, and long-term uses a username and password with server-issued REALM and NONCE. When disabled, the client remains a public Binding diagnostic.
- The deployment has a single public address and port and does not implement RFC 5780 NAT behavior discovery. Therefore CHANGE-REQUEST, PADDING, and RESPONSE-PORT remain rejected with 420, and OTHER-ADDRESS, RESPONSE-ORIGIN, and SRV service advertising remain absent.
- MAPPED-ADDRESS is not added to success responses. RFC 5389 §15.1 makes it optional and intended only for RFC 3489 backward compatibility; XOR-MAPPED-ADDRESS is the required address attribute.
- FINGERPRINT is optional in server responses and is not added unless a future usage requires it; validation is required only when a peer includes it.
- The SOFTWARE value is a stable human-readable identifier that includes the software name and version; its exact text is not fixed by this specification.
- The reference copies of RFC 5389, RFC 5769, and RFC 5780 in `docs/rfcs/` are the normative source for this audit.
- RFC 3489 backward compatibility and TCP/TLS transports are out of scope; the audited server processes UDP Binding requests.
- The server MUST implement HMAC-SHA1 for MESSAGE-INTEGRITY and CRC-32 for FINGERPRINT. If long-term credentials are selected, MD5 key derivation and SASLprep-compatible password handling are also required.
- "Known" means an attribute codepoint defined by RFC 5389 or RFC 5780 and recognized by the implementation, regardless of whether the server acts on it.
- Existing decisions in specification 007 are preserved except where this specification explicitly refines them (known-versus-unknown attribute handling, authentication, and response SOFTWARE).
