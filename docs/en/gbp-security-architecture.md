# Network Working Group                                             F000NK
# Internet-Draft                               Voluntas Progressus Team
# Intended status: Informational                                   May 2026
# Expires: November 2026

# GBP Security Architecture

## Abstract
This document defines the threat model and security architecture for the GBP stack.

## Status of This Memo
This Internet-Draft is submitted in full conformance with BCP 78 and BCP 79.
Internet-Drafts are working documents of the Internet Engineering Task Force (IETF).

## 1. Introduction
GBP uses MLS for group keying, QUIC/TLS for transport security, and protocol-specific controls for replay and authorization.

## 2. Threat Model
Adversary classes:
- Outsider network attacker
- Insider malicious member
- Compromised Delivery Service (DS)
- Compromised Authentication Service (AS)

## 3. Security Goals
- Confidentiality for media/text/control payloads
- Integrity and authenticity of all protocol messages
- Forward Secrecy (FS)
- Post-Compromise Security (PCS)
- Downgrade resistance for version/capability transitions

## 4. Trust Boundaries
AS is trusted for identity assertions but SHOULD be auditable.
DS is not trusted for confidentiality/integrity and may reorder/replay/drop.

## 5. Replay and Ordering Risk
MLS alone does not eliminate all insider replay scenarios.
Applications MUST carry unique message identifiers and enforce replay windows.

## 6. Downgrade Resistance
Endpoints MUST verify capability advertisements and MUST bind protocol version to transition metadata.
Silent downgrade MUST be treated as a policy violation.

## 7. Compromise Scenarios
### 7.1 Endpoint Compromise
Implementations SHOULD force re-initiation and key update after compromise recovery.

### 7.2 DS Compromise
System MUST detect ordering anomalies and transition divergence using digest checks.

### 7.3 AS Compromise
Deployments SHOULD support credential revocation and transparency logging.

## 8. Verification
Out-of-band epoch authenticator verification MAY be used for high-assurance sessions.

## 9. IANA Considerations
None.

## 10. Security Considerations
This document is entirely about security considerations and is normative for threat assumptions used by GBP companion drafts.

## 11. References
### 11.1 Normative References
- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels".
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words".
- [RFC9420] Barnes, R., et al., "The Messaging Layer Security (MLS) Protocol".
