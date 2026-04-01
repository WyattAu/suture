# Security Policy

## Supported Versions
Only the latest release is supported. Older versions do not receive security updates.

## Reporting a Vulnerability

If you discover a security vulnerability in Suture, please report it responsibly:

1. **Do not** open a public GitHub issue.
2. Email security vulnerabilities to: [REPLACE WITH ACTUAL EMAIL]
3. Include as much information as possible:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Any suggested fixes

We will acknowledge receipt within 48 hours and provide a timeline for the fix.

## Vulnerability Handling
- Critical vulnerabilities are addressed within 72 hours
- High severity within 1 week
- Medium severity within 2 weeks
- Low severity in the next release cycle

## Security Features
- BLAKE3 content-addressed storage with integrity verification
- Ed25519 cryptographic commit signing
- Zstd compression for blob storage
- No unsafe code in core crates (except FFI boundaries)
