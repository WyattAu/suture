# Security Policy

## Reporting a Vulnerability

Please report security vulnerabilities by email to **security@suture.dev** or by opening a
[GitHub Security Advisory](https://github.com/WyattAu/suture/security/advisories/new).

Please do not open a public issue for security vulnerabilities.

## Supported Versions

Security updates are applied to the latest release branch.

## Signing Infrastructure

Suture uses Ed25519 for cryptographic signing of commits and tags. All
key material is stored locally under `.suture/keys/` and is never transmitted
to remote servers. Keep your private keys secure and do not share them.

## Scope

The following are in scope:

- Authentication bypass (hub, platform)
- Data leakage (merge results, user data, stored patches)
- Denial of service
- RCE through WASM plugin system
- Supply chain attacks
- Privilege escalation in multi-tenant deployments

The following are out of scope:

- Self-hosted instances (user-managed security)
- Third-party dependencies (report to upstream)
- Informational issues (missing headers, cookie flags, etc.)
- Social engineering / phishing
