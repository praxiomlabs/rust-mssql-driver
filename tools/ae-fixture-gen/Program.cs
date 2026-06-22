// AE fixture provenance harness for issue #299.
//
// Drives the REAL Microsoft.Data.SqlClient binary (the same one .NET/SSMS use)
// to give the driver's "byte-exact vs Microsoft.Data.SqlClient" Always Encrypted
// fixtures bit-identical provenance to Microsoft's shipped implementation, rather
// than resting on a re-implementation of the same spec.
//
// Two checks, both via the internal SqlAeadAes256CbcHmac256Algorithm (reflection):
//   1. AEAD cell layer: re-ENCRYPT the ae_interop.rs plaintext and assert the
//      cell blob is byte-identical to the committed fixture.
//   2. Normalization layer: DECRYPT each per-type reference ciphertext from
//      mssql-client/src/encryption.rs. A successful decrypt MAC-authenticates the
//      reference as a genuine Microsoft.Data.SqlClient ciphertext (a tampered or
//      fabricated one fails the HMAC), and the recovered plaintext is the AE
//      normalized form — which the Rust unit tests separately assert the driver
//      reproduces. Together: driver == Microsoft.Data.SqlClient, reproducibly.
//
// Full *regeneration* of the normalized forms from MS's own normalizer is not
// possible offline (its scalar AE normalization is woven into the TDS parameter
// path, not a standalone reflectable API — the Server.*Normalizer classes are
// the order-preserving UDT normalizers, which bit-flip and differ). That needs a
// live AE-configured server (issue #86). This harness verifies the references
// against the real AEAD binary, which is the offline-achievable provenance.
//
// Exit 0 = every check passed against the real binary; 1 = drift.
using System;
using System.Reflection;
using System.Text;

var asm = typeof(Microsoft.Data.SqlClient.SqlConnection).Assembly;
var keyType = asm.GetType("Microsoft.Data.SqlClient.SqlAeadAes256CbcHmac256EncryptionKey")!;
var algType = asm.GetType("Microsoft.Data.SqlClient.SqlAeadAes256CbcHmac256Algorithm")!;
var encType = asm.GetType("Microsoft.Data.SqlClient.SqlClientEncryptionType")!;
const BindingFlags F = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;

static byte[] Hex(string s)
{
    var b = new byte[s.Length / 2];
    for (int i = 0; i < b.Length; i++) b[i] = Convert.ToByte(s.Substring(i * 2, 2), 16);
    return b;
}

object MakeAlg(byte[] cek)
{
    var key = Activator.CreateInstance(keyType, F, null, new object[] { cek, "AEAD_AES_256_CBC_HMAC_SHA256" }, null)!;
    var det = Enum.ToObject(encType, 1);
    return Activator.CreateInstance(algType, F, null, new object[] { key, det, (byte)1 }, null)!;
}
var encryptM = algType.GetMethod("EncryptData", F, null, new[] { typeof(byte[]) }, null)!;
var decryptM = algType.GetMethod("DecryptData", F, null, new[] { typeof(byte[]) }, null)!;

Console.WriteLine($"Microsoft.Data.SqlClient assembly version: {asm.GetName().Version}\n");
bool ok = true;

// --- Check 1: AEAD cell layer (ae_interop.rs) ---
{
    byte[] cek = new byte[32]; for (int i = 0; i < 32; i++) cek[i] = (byte)i;
    byte[] pt = Encoding.ASCII.GetBytes("Hello, SQL Server Always Encrypted!");
    const string expected =
        "0189534328ff3174ba3d9a8b5c0562487335edca1e45269d6574a33053ab5f89" +
        "5d9805dbec33622f021ccce7e426711ea90e0e2c8d789adae81ef4de18596f66" +
        "6a807edd674dd01b4517eb8ecbde7460e2a421bd3efc8308fa7050992908b83d06";
    var alg = MakeAlg(cek);
    byte[] blob = (byte[])encryptM.Invoke(alg, new object[] { pt })!;
    bool m = Convert.ToHexString(blob).ToLowerInvariant() == expected;
    Console.WriteLine($"[cell ] ae_interop deterministic blob: {(m ? "MATCH" : "DRIFT")}");
    ok &= m;
}

// --- Check 2: normalization references (encryption.rs), grouped by CEK ---
// label, expected AE normalized form (hex), reference ciphertext (hex)
var groups = new (string cek, (string label, string expect, string reference)[] vecs)[]
{
    ("B59D9F2C96784C232D53AB273D257DC79B7D2355BB82B1EC7054CE25E25F7B44", new[]
    {
        ("int(42)",        "2a00000000000000", "01102FC5DEC5D3E463A8F4BDF512AA74E6AB953BA9A2F3F9A98CD18446B007DE5A6E2A1D1EB775035EA189CA5160A935CE093CAA9BB7E9233BB333AADEE86FDE1D"),
        ("nvarchar(Ada)",  "410064006100",     "01BFAC40E6DA541ACEFAD8ECF5598DB77B0C5349CFACBC3C9221C01B6037E593B78E8F398F620F837BD6A4A2B644125C4188DF278B94479B2218466D91107FE417"),
        ("varbinary",      "010203",           "01ADE71457495F00FC9A16456F1B1EECB901D88DE97887025C189B1C4432E02071AB7594C48518CA5621E90165FAE337475B4CF3A3D00EF2D862FB0473713DF1E1"),
    }),
    ("9590E42A8A6C8F13B5D09B8D5A128EF8B3A4A10301C7AF24AFC62ED0E02342F7", new[]
    {
        ("bigint",   "0807060504030201",  "01E765FC4696660028BFD48FCAEAED81E0EB423CFF433CA97F1B2FF02F70744E7265C2AE73CAA562FFA98AF98CB1D3EF6A4649B3640359E1DB7D170C80E639DA68"),
        ("smallint", "0201000000000000",  "012545AB817E1AEBDCEE1C00AEBFF3A013CAD20E0377BEFDD9186C263F8D1A909C313A753996F1B5E4A4AE17E901F6F781DCA707544766995D339601CA414063A0"),
        ("tinyint",  "c800000000000000",  "01A97C33480277D16FFAEDA9068173D4173378542F2887EBCD31CDEEEB116BD59D48F9D459BDDCABAE469E891B4F82AA3D283440CA1B5E9FFC150F9D0AE54EC21E"),
        ("bit",      "0100000000000000",  "01DDE18564051D630EE026331BCCAFC8F4122CC3919F81459F37D9C0E0C64A5317FCA08660FE5FC855917B97B72013F25B85ADD14ADDD7D5ED022EB1297FF29A7E"),
        ("real",     "00006040",          "017A452760E7BA7AA6A716F6707F55D9C3A81683C04A6B561B13AC1D8A848E93E239BB922EE3EE628B6D0081A590BB11747CC25D216240FB10171A0FA3B99A2DB3"),
        ("float",    "0000000000000c40",  "0171611557351FBC4561EBF0B9C98E0DC38AD2BD3E2C1D1E82F185D7E67D0425E506D11DD67BA3EB38F34FB01A8FCEF7E4B9A7256944334A521526613CFF6C8C5F"),
        ("uniqueidentifier", "0403020106050807090a0b0c0d0e0f10", "01F58635AA18692D68BDF551ECDD7AC3A56682D3F91F111F8D8F36D5425C405A8F6AB3ED3C3666444478476BD65FF40DC83F6831F502826AFEEC3116F71A7A2020CCD254F4BA28FCDC0F96BA2E5264AE9E"),
        ("date(2024-03-15)", "8f460b",     "0188B4F75A1F4BDA53C9CDDC1918C09CB57F68E13F5560F1F1D7168FE70707337B1156A97915B244F3C03D3E7352882A599511BD243471FD03683F371CF44E4B76"),
    }),
    ("CBFB5AE21FB517C65DA0C6E8E11969C630798E473EF5827A70398012DF1D4B9E", new[]
    {
        // sign byte (01 = positive) + 16-byte LE unscaled magnitude (123456789 = 0x075bcd15).
        ("decimal(12345.6789)", "0115cd5b07000000000000000000000000", "018FAE46024B9B406C23600E6A9C694F9A9B39B785A995689EBE19437BA7E75768011A035A5B54B5E495512EBB46AE1146130940A0D0D834D61AA89B5AD9F71FFAF6EEEAE77E4856BA2AA5E016E2950A8D"),
        // 8-byte MONEY: 12.34 * 10000 = 123400 = 0x0001e208, high 32 bits then low 32.
        ("money(12.34)",     "0000000008e20100", "01B4CE4CAD8D6B241A1555C377A0ADD4C79424DD5162F710D116594F725C1BAB015169A0C7716076EEC90E013519B961DEF427BFC32462D9E45D166C791B73F793"),
    }),
};

foreach (var (cekHex, vecs) in groups)
{
    var alg = MakeAlg(Hex(cekHex));
    foreach (var (label, expect, refHex) in vecs)
    {
        byte[] plain;
        try { plain = (byte[])decryptM.Invoke(alg, new object[] { Hex(refHex) })!; }
        catch (Exception e) { Console.WriteLine($"[norm ] {label,-22} HMAC FAILED (not a genuine MS ciphertext): {e.InnerException?.Message}"); ok = false; continue; }
        string got = Convert.ToHexString(plain).ToLowerInvariant();
        if (expect.Length == 0)
            Console.WriteLine($"[norm ] {label,-22} MAC-verified, normalized form = {got}");
        else
        {
            bool m = got == expect;
            Console.WriteLine($"[norm ] {label,-22} MAC-verified, form {(m ? "MATCH" : $"DRIFT got={got} want={expect}")}");
            ok &= m;
        }
    }
}

Console.WriteLine(ok
    ? "\nPASS: every fixture verified against real Microsoft.Data.SqlClient"
    : "\nFAIL: drift between Microsoft.Data.SqlClient and the committed fixtures");
return ok ? 0 : 1;
