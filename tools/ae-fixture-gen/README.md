# AE fixture provenance harness (#299)

A tiny .NET console app that drives the **real `Microsoft.Data.SqlClient`
binary** to emit Always Encrypted `AEAD_AES_256_CBC_HMAC_SHA256` cell ciphertext,
and checks it byte-for-byte against the fixture committed in
[`crates/mssql-auth/tests/ae_interop.rs`](../../crates/mssql-auth/tests/ae_interop.rs).

## Why

The driver's "byte-exact vs `Microsoft.Data.SqlClient`" claim must rest on
bit-identity to Microsoft's *shipped binary*, not on a second re-implementation
of the same spec. The Rust unit tests prove `Rust encoder == fixture`; the
Python generator (`crates/mssql-auth/tests/generate_ae_fixtures.py`) proves
`fixture == an independent transcription of the spec`. Neither proves
`fixture == Microsoft's binary`. This harness closes that gap: it invokes the
internal `SqlAeadAes256CbcHmac256Algorithm` (by reflection — it is not public
API) and asserts its output equals the committed fixture. Together the three
give `Rust == Microsoft.Data.SqlClient`, reproducibly.

## Run

Requires the .NET SDK (8.0+); no SQL Server needed — only the encryption
library's algorithm is exercised.

```bash
dotnet run --project tools/ae-fixture-gen
```

Exit code `0` means the real binary is byte-identical to `ae_interop.rs`; `1`
means drift (the fixture, the Rust encoder, or the bundled `Microsoft.Data.SqlClient`
version disagree — investigate before trusting the claim). The app also prints
the Rust `SPEC_DETERMINISTIC_BLOB` array form, for regenerating the fixture if
the test inputs ever change.

The pinned `Microsoft.Data.SqlClient` version is in
[`AeFixtureGen.csproj`](AeFixtureGen.csproj) (5.2.2 at the time of writing —
the version the `ae_interop.rs` provenance note refers to). Bump it deliberately
and re-run; a passing run after a bump extends the provenance to that version.

## Scope

This validates the **AEAD cell-encryption layer** (the `ae_interop.rs` fixture).
The per-type normalization reference ciphertexts in
`crates/mssql-client/src/encryption.rs` are still backed by values captured
manually from a live `Microsoft.Data.SqlClient` INSERT; extending this harness to
regenerate those is a follow-up.
