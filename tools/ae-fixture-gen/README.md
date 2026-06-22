# AE fixture provenance harness (#299)

A tiny .NET console app that drives the **real `Microsoft.Data.SqlClient`
binary** to give the driver's Always Encrypted "byte-exact vs
`Microsoft.Data.SqlClient`" fixtures bit-identical provenance to Microsoft's
shipped implementation — rather than resting on a re-implementation of the same
spec.

## Why

The claim must rest on bit-identity to Microsoft's *shipped binary*, not on a
second implementation of the same spec. The Rust unit tests prove
`Rust encoder == fixture`; the Python generator
(`crates/mssql-auth/tests/generate_ae_fixtures.py`) proves
`fixture == an independent transcription of the spec`. Neither proves
`fixture == Microsoft's binary`. This harness closes that gap by invoking the
internal `SqlAeadAes256CbcHmac256Algorithm` (by reflection — it is not public
API).

## What it checks

Both checks run against the real binary's AEAD algorithm; **no SQL Server
required** (only the encryption library is exercised).

1. **AEAD cell layer** — re-encrypts the `ae_interop.rs` plaintext and asserts the
   deterministic cell blob is byte-identical to the committed
   `SPEC_DETERMINISTIC_BLOB`.
2. **Normalization references** — decrypts each per-type reference ciphertext from
   [`crates/mssql-client/src/encryption.rs`](../../crates/mssql-client/src/encryption.rs)
   (int / nvarchar / varbinary / bigint / smallint / tinyint / bit / real / float /
   uniqueidentifier / date / decimal / money). A successful decrypt
   **MAC-authenticates** the reference as a genuine `Microsoft.Data.SqlClient`
   ciphertext (a tampered or fabricated one fails the HMAC), and the recovered
   plaintext is asserted to equal the documented AE normalized form. The Rust unit
   tests separately assert the driver reproduces those same forms — together,
   `driver == Microsoft.Data.SqlClient`.

## Run

```bash
dotnet run --project tools/ae-fixture-gen
```

Exit `0` = every fixture verified against the real binary; `1` = drift (a fixture,
the Rust encoder, or the bundled `Microsoft.Data.SqlClient` version disagree —
investigate before trusting the claim).

The pinned `Microsoft.Data.SqlClient` version is in
[`AeFixtureGen.csproj`](AeFixtureGen.csproj) (5.2.2 — the version the
`ae_interop.rs` provenance note refers to). Bump it deliberately and re-run; a
passing run after a bump extends the provenance to that version.

## What this does NOT do (remaining gap → #86)

It **verifies** the reference ciphertexts against the real AEAD binary; it does
not **regenerate the normalized forms from Microsoft's own normalizer**. That is
not achievable offline: MS's scalar AE normalization is woven into the TDS
parameter-writing path, not a standalone reflectable API (the
`Microsoft.Data.SqlClient.Server.*Normalizer` classes are the order-preserving
UDT normalizers — they bit-flip for sortability and differ). Regenerating from
scratch needs a live AE-configured SQL Server (a CMK/CEK + an encrypted-column
INSERT), which is issue #86's territory. The temporal forms
(`ae_normalization_matches_dotnet_temporal`) are asserted directly as normalized
bytes in the Rust tests and are not re-verified here.
