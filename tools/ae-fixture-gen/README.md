# AE fixture provenance harness (#299, #86)

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

## Live mode — regenerate from Microsoft's live normalizer (#86)

The offline checks above *verify* pre-captured references but cannot *regenerate*
the normalized forms from Microsoft's own normalizer — its scalar AE
normalization is woven into the TDS parameter-write path, not a reflectable API.
Live mode closes that gap against a real SQL Server:

```bash
just sql-server-start   # local SQL Server 2022 — Always Encrypted is supported in all editions
MSSQL_PASSWORD='YourStrong@Passw0rd' dotnet run --project tools/ae-fixture-gen -- live
```

(env: `AE_LIVE_HOST` (default `localhost,1433`), `MSSQL_USER` (`sa`), `MSSQL_PASSWORD`.)

It provisions CMK/CEK + an encrypted table, inserts each raw value through the
real `Microsoft.Data.SqlClient` client-side encryption (which runs MS's
normalizer, then AEAD-encrypts), reads the ciphertext back over a plain
connection, and:

- byte-compares it to the committed reference ciphertext for the scalar types
  (under the same CEKs); and
- decrypts it to recover Microsoft's normalized form and compares to the
  committed form for **every** type — including the temporal (`time` /
  `datetime2` / `datetimeoffset` at scales 7 and 3, legacy `datetime`,
  `smalldatetime`) and fixed-width (`char` / `nchar` / `binary`) forms the
  offline check cannot reach.

No certificate / Key Vault provisioning is needed: SQL Server stores the
encrypted CEK opaquely (it has no CMK to validate it) and a custom in-process
key store provider is trusted to unwrap it — so a stub provider returning a
fixed CEK, paired with a dummy `ENCRYPTED_VALUE`, drives the real client-side
path. Exit `0` = every form reproduced by the live normalizer; `1` = drift.
