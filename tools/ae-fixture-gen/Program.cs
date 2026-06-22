// AE fixture provenance harness for issue #299.
//
// Drives the REAL Microsoft.Data.SqlClient binary (the same one .NET/SSMS use)
// to emit Always Encrypted AEAD_AES_256_CBC_HMAC_SHA256 cell ciphertext, then
// checks it byte-for-byte against the fixture committed in
// crates/mssql-auth/tests/ae_interop.rs. This gives the "byte-exact vs
// Microsoft.Data.SqlClient" claim bit-identical provenance to Microsoft's
// shipped implementation, rather than resting on a second re-implementation of
// the same spec (generate_ae_fixtures.py).
//
// The internal AEAD classes are reached by reflection (they are not public API).
// Exit code 0 = the real binary matches the committed fixture; 1 = drift.
using System;
using System.Reflection;
using System.Text;

const string AlgName = "AEAD_AES_256_CBC_HMAC_SHA256";
const string PlaintextStr = "Hello, SQL Server Always Encrypted!";

// Must match CEK + PLAINTEXT + SPEC_DETERMINISTIC_BLOB in tests/ae_interop.rs.
byte[] cek = new byte[32];
for (int i = 0; i < 32; i++) cek[i] = (byte)i;
byte[] plaintext = Encoding.ASCII.GetBytes(PlaintextStr);
const string ExpectedHex =
    "0189534328ff3174ba3d9a8b5c0562487335edca1e45269d6574a33053ab5f89" +
    "5d9805dbec33622f021ccce7e426711ea90e0e2c8d789adae81ef4de18596f66" +
    "6a807edd674dd01b4517eb8ecbde7460e2a421bd3efc8308fa7050992908b83d06";

var asm = typeof(Microsoft.Data.SqlClient.SqlConnection).Assembly;
var keyType = asm.GetType("Microsoft.Data.SqlClient.SqlAeadAes256CbcHmac256EncryptionKey")!;
var algType = asm.GetType("Microsoft.Data.SqlClient.SqlAeadAes256CbcHmac256Algorithm")!;
var encType = asm.GetType("Microsoft.Data.SqlClient.SqlClientEncryptionType")!;
const BindingFlags Flags = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;

var key = Activator.CreateInstance(keyType, Flags, null, new object[] { cek, AlgName }, null)!;
var deterministic = Enum.ToObject(encType, 1); // SqlClientEncryptionType.Deterministic
var alg = Activator.CreateInstance(algType, Flags, null, new object[] { key, deterministic, (byte)1 }, null)!;
var encryptData = algType.GetMethod("EncryptData", Flags, null, new[] { typeof(byte[]) }, null)!;
var decryptData = algType.GetMethod("DecryptData", Flags, null, new[] { typeof(byte[]) }, null)!;

byte[] blob = (byte[])encryptData.Invoke(alg, new object[] { plaintext })!;
byte[] roundtrip = (byte[])decryptData.Invoke(alg, new object[] { blob })!;
string blobHex = Convert.ToHexString(blob).ToLowerInvariant();

Console.WriteLine($"Microsoft.Data.SqlClient assembly version: {asm.GetName().Version}");
Console.WriteLine($"deterministic cell blob ({blob.Length} bytes): {blobHex}");
Console.WriteLine($"round-trip decrypt ok: {Encoding.ASCII.GetString(roundtrip) == PlaintextStr}");

// Emit the Rust array form, for regenerating the fixture if it ever needs it.
var sb = new StringBuilder("const SPEC_DETERMINISTIC_BLOB: [u8; ").Append(blob.Length).Append("] = [\n");
for (int i = 0; i < blob.Length; i++)
{
    if (i % 16 == 0) sb.Append("    ");
    sb.Append("0x").Append(blob[i].ToString("x2")).Append(',');
    sb.Append(i % 16 == 15 || i == blob.Length - 1 ? "\n" : " ");
}
sb.Append("];");
Console.WriteLine("\n// Rust fixture form:\n" + sb);

bool ok = blobHex == ExpectedHex && Encoding.ASCII.GetString(roundtrip) == PlaintextStr;
Console.WriteLine(ok
    ? "\nPASS: real Microsoft.Data.SqlClient output is byte-identical to ae_interop.rs"
    : "\nFAIL: drift between Microsoft.Data.SqlClient and the committed fixture");
return ok ? 0 : 1;
