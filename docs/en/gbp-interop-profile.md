# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Standards Track                                 May 2026
# Expires: November 2026

# GBP Interoperability Profile

## Abstract
This document defines conformance classes and interoperability requirements for GBP implementations.

## Status of This Memo
This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.
Internet-Drafts are working documents of the Internet Engineering Task Force (IETF).

## 1. Introduction
The profile defines minimum feature sets required for compliant interoperation.

## 2. Conventions
BCP 14 words apply.

## 3. Conformance Classes
- Class A: GBP + GSP required
- Class B: Class A + GTP required
- Class C: Class B + GAP required

Implementations MUST declare conformance class at capability negotiation time.

## 4. Mandatory Features
- QUIC + TLS 1.3 transport
- MLS epoch processing
- GBP-Control transition procedures
- Error registry support with machine-readable NACK
- Replay window enforcement

## 5. Optional Features
- Attachment out-of-band transport
- Extended signal registry ranges
- Out-of-band epoch authenticator UX

## 6. Version Negotiation
Endpoints MUST advertise:
- supported GBP version range
- supported subprotocol versions
- optional feature flags

Version intersection failure MUST terminate handshake.

## 7. Test Vectors and Checklist
Required interop tests:
1. Initial group formation.
2. Add/remove member transition.
3. Concurrent commit tie-break.
4. Invalid commit recovery.
5. Replay rejection.
6. Gap overlap key decryption.
7. GTP idempotent duplicate handling.
8. GSP authorization rejection with NACK.

## 8. Compliance Output
Implementations SHOULD expose a conformance report listing pass/fail per checklist item.

## 9. IANA Considerations
No new IANA actions.

## 10. Security Considerations
Interop shortcuts MUST NOT weaken MLS invariants, replay controls, or downgrade resistance.

## 11. References
### 11.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
