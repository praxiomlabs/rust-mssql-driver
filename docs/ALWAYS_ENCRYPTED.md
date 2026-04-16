# Always Encrypted

SQL Server Always Encrypted protects sensitive data so that plaintext is never
visible to SQL Server itself. Encryption and decryption happen entirely on the
client side.

## Quick Start

```rust,ignore
use mssql_client::{Client, Config};
use mssql_auth::key_store::InMemoryKeyStore;

// 1. Build an encryption config with a key store provider
let mut key_store = InMemoryKeyStore::new();
key_store.add_key("CMK_Auto1", cek_bytes); // 32-byte Column Encryption Key

let encryption_config = mssql_client::EncryptionConfig::builder()
    .add_provider("MSSQL_CERTIFICATE_STORE", key_store)
    .build();

// 2. Connect with encryption enabled
let config = Config::from_connection_string(
    "Server=localhost;Database=mydb;User Id=sa;Password=secret;\
     Column Encryption Setting=Enabled"
)?
.with_column_encryption(encryption_config);

let mut client = Client::connect(config).await?;

// 3. Query normally — decryption is transparent
let rows = client.query("SELECT SSN, Name FROM Patients", &[]).await?;
for row in rows {
    let ssn: String = row.get(0)?; // decrypted automatically
    println!("{ssn}");
}
```

## How It Works

1. **Login**: The client negotiates Always Encrypted support via the
   `FEATURE_EXT_COLUMNENCRYPTION` flag in the LOGIN7 packet.

2. **ColMetaData**: When a result set contains encrypted columns, SQL Server
   sends a `CekTable` (Column Encryption Key table) and per-column
   `CryptoMetadata` inside the `ColMetaData` token.

3. **Key Resolution**: The client resolves each CEK by calling the registered
   `KeyStoreProvider` for the CMK's provider name (e.g., `AZURE_KEY_VAULT`).
   This step is **async** because providers may perform network I/O.

4. **Row Decryption**: Each encrypted column value is decrypted using
   AEAD_AES_256_CBC_HMAC_SHA256. This step is **synchronous** in the row
   parsing hot path — CEKs were pre-resolved in step 3.

5. **Type Reconstruction**: Decrypted bytes are re-parsed according to the
   column's base type (the real type, not the wire type `BigVarBinary`).

## Key Store Providers

### InMemoryKeyStore

For development and testing. You provide the raw CEK bytes directly.

```rust,ignore
use mssql_auth::key_store::InMemoryKeyStore;

let mut store = InMemoryKeyStore::new();
store.add_key("MyCMK", raw_cek_bytes);
```

### AzureKeyVaultProvider

For production with Azure Key Vault. Requires the `azure-identity` feature.

```rust,ignore
use mssql_auth::azure_keyvault::AzureKeyVaultProvider;

let provider = AzureKeyVaultProvider::new(
    azure_identity::DefaultAzureCredential::new()?,
);
```

### WindowsCertStoreProvider

For production with the Windows Certificate Store. Requires the `sspi-auth`
feature and Windows.

```rust,ignore
use mssql_auth::windows_certstore::WindowsCertStoreProvider;

let provider = WindowsCertStoreProvider::new();
```

### Custom Providers

Implement the `KeyStoreProvider` trait for custom key storage:

```rust,ignore
use mssql_auth::key_store::KeyStoreProvider;

struct MyHsmProvider { /* ... */ }

#[async_trait::async_trait]
impl KeyStoreProvider for MyHsmProvider {
    async fn decrypt_column_encryption_key(
        &self,
        master_key_path: &str,
        encryption_algorithm: &str,
        encrypted_column_encryption_key: &[u8],
    ) -> Result<Vec<u8>, mssql_auth::EncryptionError> {
        // Call your HSM here
        todo!()
    }
}
```

## Connection String Keywords

| Keyword | Values | Effect |
|---------|--------|--------|
| `Column Encryption Setting` | `Enabled` / `Disabled` | Enables/disables Always Encrypted |

## Encryption Types

| Type | Wire Value | Behavior |
|------|------------|----------|
| **Deterministic** | 1 | Same plaintext always produces the same ciphertext. Supports equality comparisons, joins, and indexing. |
| **Randomized** | 2 | Same plaintext produces different ciphertext each time. More secure but does not support server-side comparisons. |

## Supported Algorithm

Currently only `AEAD_AES_256_CBC_HMAC_SHA256` is supported (algorithm ID 2).
This is the only algorithm defined by the MS-TDS specification for Always
Encrypted.

## Scope

Transparent decryption is supported in all query paths:

- `client.query()` / `client.query_stream()`
- `client.call_procedure()` / `client.procedure().execute()`
- `client.query_multiple()`

Live-server validation (SQL Server 2022) covers:

- LOGIN7 `FEATURE_EXT_COLUMNENCRYPTION` negotiation succeeds with any number
  of `Config` clones in the retry/redirect path. (Fixes two critical bugs
  that prevented any AE connection from succeeding in v0.5.x – v0.9.x — see
  CHANGELOG.md for v0.10.0.)
- `ColMetaData` parsing of `CekTable` + per-column `CryptoMetadata`
- Async CEK resolution through registered `KeyStoreProvider`s
- AEAD_AES_256_CBC_HMAC_SHA256 decryption in the row-parsing hot path
- `NULL` encrypted column values surface as `Option<T>::None` without
  attempting AEAD decryption

Plaintext ciphertext round-trip (non-NULL writes into encrypted columns)
is blocked on the parameter encryption write path — see
[Limitations](#limitations) below.

## Security Considerations

- **Key material never reaches SQL Server.** CEKs are decrypted client-side
  using the Column Master Key (CMK) from the configured key store.
- **HMAC verification** is performed before decryption. Tampered ciphertext
  is rejected — garbled data is never returned.
- **Error messages never contain key material** or plaintext values.
- **Do NOT use `ENCRYPTBYKEY`** — it is a server-side encryption function
  with a fundamentally different threat model. See CLAUDE.md for details.

## Limitations

- **Parameter encryption (write path) is not yet implemented.**
  Sending a non-`NULL` plaintext value into an encrypted column via an
  RPC parameter is rejected by SQL Server with
  *"Operand type clash: varchar is incompatible with varchar(n) encrypted
  with (…)."* This affects `INSERT`, `UPDATE`, and any `WHERE`-clause
  equality comparison against a deterministic-encrypted column.

  What this release *does* support:

  - Reading encrypted columns and decrypting them transparently.
  - Writing `NULL` into a nullable encrypted column.
  - Writing ciphertext that was produced by a different client (e.g.,
    .NET SqlClient) — decryption on read works the same.

  The missing piece is the client-side *encrypt-before-send* logic:
  the driver needs to call `sp_describe_parameter_encryption` (or accept
  caller-supplied `CryptoMetadata`), encrypt the plaintext parameter
  value with `AeadEncryptor`, and send it with `fStatusFlags.encrypted = 1`
  along with a CEK table referencing the column encryption key. The
  low-level primitives (`AeadEncryptor::encrypt`, `CekTable`, the
  `encrypted` param flag) are all implemented; the orchestration layer
  is not.

  Workaround for now: pre-encrypt values client-side using a separate
  tool (e.g., .NET SqlClient or `sqlcmd`) and insert the resulting
  ciphertext directly as a `VARBINARY` parameter bound to the base
  column type. This is not transparent — the column must be queried
  with `Column Encryption Setting=Enabled` to decrypt on read.

- Key rotation requires restarting the connection (CEKs are cached per
  result set, not globally).
