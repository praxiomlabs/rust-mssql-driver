# mssql-auth

> Part of the [rust-mssql-driver](../../README.md) project.

Authentication strategies for SQL Server connections.

## Overview

This crate provides various authentication methods for SQL Server, isolated from connection logic for better modularity and testing. It implements a tiered authentication system designed for different deployment scenarios.

## Authentication Tiers

### Tier 1: Core (Pure Rust, Default)

| Method | Description |
|--------|-------------|
| `SqlServerAuth` | Username/password via LOGIN7 |
| `AzureAdAuth` | Pre-acquired access token |

### Tier 2: Azure Native (`azure-identity` feature)

| Method | Description |
|--------|-------------|
| Managed Identity | Azure VM/Container identity |
| Service Principal | Client ID + Secret |

### Tier 3: Enterprise/Legacy (`integrated-auth` feature)

| Method | Description |
|--------|-------------|
| Kerberos | Linux/macOS via GSSAPI |
| NTLM/Kerberos | Windows via SSPI |

## Usage

```rust
use mssql_auth::{SqlServerAuth, AzureAdAuth, AuthProvider};

// SQL Server authentication
let sql_auth = SqlServerAuth::new("sa", "Password123!");
let auth_data = sql_auth.authenticate().unwrap();

// Azure AD authentication with pre-acquired token
let azure_auth = AzureAdAuth::with_token("eyJ0eXAi...");
```

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `azure-identity` | No | Azure Managed Identity and Service Principal support |
| `integrated-auth` | No | Kerberos/SPNEGO via libgssapi (Linux/macOS) |
| `sspi-auth` | No | Windows SSPI authentication (cross-platform via sspi-rs) |
| `cert-auth` | No | Client certificate authentication (Azure AD with X.509) |
| `zeroize` | No | Secure credential zeroization on drop |
| `always-encrypted` | No | Always Encrypted client-side encryption support |

## Secure Credential Handling

Enable the `zeroize` feature for secure credential handling:

```toml
mssql-auth = { version = "0.11", features = ["zeroize"] }
```

This automatically zeroes sensitive data from memory when credentials are dropped.

## Modules

| Module | Description |
|--------|-------------|
| `azure_ad` | Azure AD and federated authentication |
| `credentials` | Credential types and secure handling |
| `error` | Authentication error types |
| `provider` | `AuthProvider` trait and `AuthData` |
| `sql_auth` | SQL Server username/password auth |

## Security Considerations

- Never log credentials or access tokens
- Use `zeroize` feature in production for sensitive environments
- Prefer Managed Identity over Service Principal when possible
- Client certificates provide mutual TLS authentication

## License

MIT OR Apache-2.0
