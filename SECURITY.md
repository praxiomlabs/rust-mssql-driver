# Security Policy

## Supported Versions

The latest released minor version receives security fixes; pre-1.0, older minors
are end-of-life. See [STABILITY.md § Security Support](STABILITY.md#security-support).

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.** Report them
privately via [GitHub Security Advisories](https://github.com/praxiomlabs/rust-mssql-driver/security/advisories/new).

Please include: a description of the vulnerability, its impact, steps to
reproduce, and affected versions. Aim of the response process:

- **Acknowledgment** within 48 hours
- **Initial assessment** within 7 days
- **Fix and disclosure** coordinated with the reporter, prioritized by severity

## Security Posture

The driver's threat model, trust boundaries, and the TLS/cryptography internals
are documented where they live:

- **TLS / transport security** — encryption modes (`Encrypt=strict`/`true`/`no_tls`),
  certificate validation, TLS versions, cipher suites, TDS 8.0 strict mode:
  see the [`mssql-tls` crate docs](https://docs.rs/mssql-tls).
- **Always Encrypted** — client-side column encryption, key providers, the
  cryptographic design: see the [`mssql-client` `encryption` module docs](https://docs.rs/mssql-client/latest/mssql_client/encryption/)
  and [ARCHITECTURE.md § ADR-013](ARCHITECTURE.md).

In brief: the driver assumes a hostile network (mitigated by TLS with certificate
validation on by default) and a trusted-by-default server. For threat models that
include a malicious DBA or server compromise, use Always Encrypted (below), which
keeps data encrypted even from the server.

## Secure Defaults

| Setting | Default | Insecure option |
|---------|---------|-----------------|
| TLS | Enabled | `Encrypt=false` (logs warning) |
| Certificate validation | Enabled | `TrustServerCertificate=true` (logs warning) |
| Parameter binding | Required | N/A — no raw SQL interpolation API |
| Connection timeout | 15 seconds | `Connect Timeout=0` (infinite) |
| Command timeout | 30 seconds | `Command Timeout=0` (infinite) |

## Credential Handling

- Passwords are never logged; connection strings are sanitized in error messages.
- The optional `zeroize` feature securely wipes credentials from memory on drop.

## SQL Injection

- Parameters are bound via the TDS RPC protocol, never string-interpolated — there
  is no raw-interpolation API.
- Savepoint names are validated (`validate_identifier`) so developer-supplied
  identifiers can't become an injection vector.

## Always Encrypted

For data that must stay encrypted even on the SQL Server, the `always-encrypted`
feature provides client-side encryption (AEAD_AES_256_CBC_HMAC_SHA256, RSA-OAEP key
unwrapping, CEK caching) with `InMemoryKeyStore`, `AzureKeyVaultProvider`,
`WindowsCertStoreProvider`, or a custom `KeyStoreProvider`.

| Threat | Standard TLS | Always Encrypted |
|--------|--------------|------------------|
| Network attackers | Protected | Protected |
| Compromised DBAs | Exposed | Protected |
| Server memory access | Exposed | Protected |
| Backup theft | Exposed | Protected |

**Do NOT use T-SQL `ENCRYPTBYKEY` as a substitute** — its keys exist on the server,
so it does not protect against a malicious DBA or server compromise the way Always
Encrypted does. See the [`mssql-client` `encryption` module docs](https://docs.rs/mssql-client/latest/mssql_client/encryption/).

## Dependency Security

Dependencies are audited with `cargo deny check` (RustSec advisories, unmaintained
crates, license compliance) in CI.
