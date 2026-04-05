# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Suture, please report it privately
by opening a [GitHub Security Advisory](https://github.com/WyattAu/suture/security/advisories/new).

Please do not open a public issue for security vulnerabilities.

## Supported Versions

Only the latest release of Suture receives security updates.

## Signing Infrastructure

Suture uses Ed25519 for cryptographic signing of commits and tags. All
key material is stored locally under `.suture/keys/` and is never transmitted
to remote servers. Keep your private keys secure and do not share them.
