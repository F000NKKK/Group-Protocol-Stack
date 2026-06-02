# Security Policy

## Supported Versions

| Version      | Supported          |
| ------------ | ------------------ |
| 1.7.x        | :white_check_mark: |
| < 1.7.0      | :x:                |

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Use GitHub's [private vulnerability reporting](https://github.com/F000NKKK/Group-Protocol-Stack/security/advisories/new)
to submit a report confidentially.

Alternatively, email [burtelgamerpro@gmail.com](mailto:burtelgamerpro@gmail.com)
with the subject line `[SECURITY] Group-Protocol-Stack`.

### What to include

- A clear description of the vulnerability and its impact
- Steps to reproduce (proof-of-concept code if possible)
- Affected versions
- Any suggested mitigations

### Response timeline

| Stage | Target |
| ----- | ------ |
| Initial acknowledgement | 48 hours |
| Triage and severity assessment | 5 business days |
| Fix or workaround available | 30 days (critical), 90 days (others) |
| Public disclosure | After fix is released |

We follow [responsible disclosure](https://en.wikipedia.org/wiki/Coordinated_vulnerability_disclosure):
we will coordinate a public disclosure date with you after the fix is ready.

### Scope

This policy covers vulnerabilities in the GBP/GTP/GAP/GSP protocol
implementations, cryptographic primitives, MLS integration, SFrame
media encryption, and the FFI layer exposed to .NET, Node.js, and Python.

Out of scope: vulnerabilities in third-party dependencies (report those
upstream), build tooling, and documentation.
