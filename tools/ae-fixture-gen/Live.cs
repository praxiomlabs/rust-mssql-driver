// Live AE-normalizer capture (issue #86).
//
// The offline harness (Program.cs) can only *verify* pre-captured reference
// ciphertexts against the Microsoft.Data.SqlClient AEAD binary. It cannot run
// Microsoft's scalar AE *normalizer* on a raw value — that normalization is
// woven into the TDS parameter-write path, not a reflectable API. In
// particular the temporal and fixed-width normalized forms are asserted in the
// Rust tests as raw bytes with no offline MS cross-check.
//
// This path closes that gap against a live SQL Server: it inserts raw values
// through the real Microsoft.Data.SqlClient client-side encryption (which
// normalizes then AEAD-encrypts), reads the ciphertext back over a plain
// connection, and (a) byte-compares it to the committed fixture for the scalar
// types whose CEK we replicate, and (b) decrypts it to recover Microsoft's
// normalized form and compares to the expected normalized bytes for every type.
//
// Key insight that removes all envelope crypto: SQL Server stores the encrypted
// CEK opaquely (no CMK to validate it) and a *custom* key store provider is
// trusted to unwrap it — so a stub provider returning a fixed CEK, paired with
// a dummy ENCRYPTED_VALUE, drives the real client-side path.
using System;
using System.Collections.Generic;
using System.Data;
using System.Reflection;
using Microsoft.Data.SqlClient;

static class Live
{
    sealed class StubKeyStoreProvider : SqlColumnEncryptionKeyStoreProvider
    {
        private readonly Dictionary<string, byte[]> _byPath;
        public StubKeyStoreProvider(Dictionary<string, byte[]> byPath) => _byPath = byPath;
        public override byte[] DecryptColumnEncryptionKey(string masterKeyPath, string a, byte[] e) => _byPath[masterKeyPath];
        public override byte[] EncryptColumnEncryptionKey(string p, string a, byte[] k) => new byte[] { 1, 2, 3, 4, 5, 6, 7, 8 };
    }

    static byte[] Hex(string s)
    {
        var b = new byte[s.Length / 2];
        for (int i = 0; i < b.Length; i++) b[i] = Convert.ToByte(s.Substring(i * 2, 2), 16);
        return b;
    }
    static string ToHex(byte[] b) => Convert.ToHexString(b).ToLowerInvariant();

    static Func<byte[], byte[]> MakeDecryptor(byte[] cek)
    {
        var asm = typeof(SqlConnection).Assembly;
        var keyType = asm.GetType("Microsoft.Data.SqlClient.SqlAeadAes256CbcHmac256EncryptionKey")!;
        var algType = asm.GetType("Microsoft.Data.SqlClient.SqlAeadAes256CbcHmac256Algorithm")!;
        var encType = asm.GetType("Microsoft.Data.SqlClient.SqlClientEncryptionType")!;
        const BindingFlags F = BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic;
        var key = Activator.CreateInstance(keyType, F, null, new object[] { cek, "AEAD_AES_256_CBC_HMAC_SHA256" }, null)!;
        var det = Enum.ToObject(encType, 1);
        var alg = Activator.CreateInstance(algType, F, null, new object[] { key, det, (byte)1 }, null)!;
        var decryptM = algType.GetMethod("DecryptData", F, null, new[] { typeof(byte[]) }, null)!;
        return ct => (byte[])decryptM.Invoke(alg, new object[] { ct })!;
    }

    static void Exec(SqlConnection c, string sql)
    {
        using var cmd = c.CreateCommand();
        cmd.CommandText = sql;
        cmd.ExecuteNonQuery();
    }

    // One value to round-trip: a column DDL fragment, the parameter, the CEK
    // group, the expected normalized form, and (scalars only) the committed
    // reference ciphertext to byte-match.
    sealed class Case
    {
        public required string Col;
        public required string SqlType;       // column type, e.g. "INT" or "DATETIME2(7)"
        public string Collate = "";           // e.g. " COLLATE Latin1_General_BIN2" for string types
        public required int CekGroup;         // 0..2
        public required SqlParameter Param;
        public required string Label;
        public required string Form;          // expected normalized form (hex)
        public string Ct = "";                // committed reference ciphertext (hex), scalars only
    }

    static SqlParameter P(SqlDbType t, object v, byte scale = 0, byte precision = 0, int size = 0)
    {
        var p = new SqlParameter("@p", t) { Value = v };
        if (scale != 0) p.Scale = scale;
        if (precision != 0) p.Precision = precision;
        if (size != 0) p.Size = size;
        return p;
    }

    public static int Run(string server, string user, string password)
    {
        string[] ceks =
        {
            "B59D9F2C96784C232D53AB273D257DC79B7D2355BB82B1EC7054CE25E25F7B44",
            "9590E42A8A6C8F13B5D09B8D5A128EF8B3A4A10301C7AF24AFC62ED0E02342F7",
            "CBFB5AE21FB517C65DA0C6E8E11969C630798E473EF5827A70398012DF1D4B9E",
        };
        var decryptors = new Func<byte[], byte[]>[3];
        for (int i = 0; i < 3; i++) decryptors[i] = MakeDecryptor(Hex(ceks[i]));

        var ts = new TimeSpan(0, 13, 14, 15).Add(TimeSpan.FromTicks(1234567)); // 13:14:15.1234567
        var dt = new DateTime(2024, 3, 15, 13, 14, 15).AddTicks(1234567);
        var dto = new DateTimeOffset(2024, 3, 15, 13, 14, 15, TimeSpan.FromMinutes(330)).AddTicks(1234567);
        var dtLegacy = new DateTime(2024, 3, 15, 13, 14, 15).AddMilliseconds(123);
        var sdt = new DateTime(2024, 3, 15, 13, 14, 0);

        var cases = new List<Case>
        {
            // Group 0 (CEK B59D…) — scalar, ciphertext byte-match
            new() { Col="c_int",  SqlType="INT", CekGroup=0, Param=P(SqlDbType.Int, 42), Label="int(42)", Form="2a00000000000000", Ct="01102FC5DEC5D3E463A8F4BDF512AA74E6AB953BA9A2F3F9A98CD18446B007DE5A6E2A1D1EB775035EA189CA5160A935CE093CAA9BB7E9233BB333AADEE86FDE1D" },
            new() { Col="c_nv",   SqlType="NVARCHAR(50)", Collate=" COLLATE Latin1_General_BIN2", CekGroup=0, Param=P(SqlDbType.NVarChar, "Ada", size:50), Label="nvarchar(Ada)", Form="410064006100", Ct="01BFAC40E6DA541ACEFAD8ECF5598DB77B0C5349CFACBC3C9221C01B6037E593B78E8F398F620F837BD6A4A2B644125C4188DF278B94479B2218466D91107FE417" },
            new() { Col="c_vb",   SqlType="VARBINARY(50)", CekGroup=0, Param=P(SqlDbType.VarBinary, new byte[]{1,2,3}, size:50), Label="varbinary", Form="010203", Ct="01ADE71457495F00FC9A16456F1B1EECB901D88DE97887025C189B1C4432E02071AB7594C48518CA5621E90165FAE337475B4CF3A3D00EF2D862FB0473713DF1E1" },

            // Group 1 (CEK 9590…) — scalar, ciphertext byte-match
            new() { Col="c_bigint", SqlType="BIGINT", CekGroup=1, Param=P(SqlDbType.BigInt, 0x0102030405060708L), Label="bigint", Form="0807060504030201", Ct="01E765FC4696660028BFD48FCAEAED81E0EB423CFF433CA97F1B2FF02F70744E7265C2AE73CAA562FFA98AF98CB1D3EF6A4649B3640359E1DB7D170C80E639DA68" },
            new() { Col="c_smallint", SqlType="SMALLINT", CekGroup=1, Param=P(SqlDbType.SmallInt, (short)258), Label="smallint", Form="0201000000000000", Ct="012545AB817E1AEBDCEE1C00AEBFF3A013CAD20E0377BEFDD9186C263F8D1A909C313A753996F1B5E4A4AE17E901F6F781DCA707544766995D339601CA414063A0" },
            new() { Col="c_tinyint", SqlType="TINYINT", CekGroup=1, Param=P(SqlDbType.TinyInt, (byte)200), Label="tinyint", Form="c800000000000000", Ct="01A97C33480277D16FFAEDA9068173D4173378542F2887EBCD31CDEEEB116BD59D48F9D459BDDCABAE469E891B4F82AA3D283440CA1B5E9FFC150F9D0AE54EC21E" },
            new() { Col="c_bit", SqlType="BIT", CekGroup=1, Param=P(SqlDbType.Bit, true), Label="bit", Form="0100000000000000", Ct="01DDE18564051D630EE026331BCCAFC8F4122CC3919F81459F37D9C0E0C64A5317FCA08660FE5FC855917B97B72013F25B85ADD14ADDD7D5ED022EB1297FF29A7E" },
            new() { Col="c_real", SqlType="REAL", CekGroup=1, Param=P(SqlDbType.Real, 3.5f), Label="real", Form="00006040", Ct="017A452760E7BA7AA6A716F6707F55D9C3A81683C04A6B561B13AC1D8A848E93E239BB922EE3EE628B6D0081A590BB11747CC25D216240FB10171A0FA3B99A2DB3" },
            new() { Col="c_float", SqlType="FLOAT", CekGroup=1, Param=P(SqlDbType.Float, 3.5d), Label="float", Form="0000000000000c40", Ct="0171611557351FBC4561EBF0B9C98E0DC38AD2BD3E2C1D1E82F185D7E67D0425E506D11DD67BA3EB38F34FB01A8FCEF7E4B9A7256944334A521526613CFF6C8C5F" },
            new() { Col="c_uuid", SqlType="UNIQUEIDENTIFIER", CekGroup=1, Param=P(SqlDbType.UniqueIdentifier, new Guid("01020304-0506-0708-090a-0b0c0d0e0f10")), Label="uniqueidentifier", Form="0403020106050807090a0b0c0d0e0f10", Ct="01F58635AA18692D68BDF551ECDD7AC3A56682D3F91F111F8D8F36D5425C405A8F6AB3ED3C3666444478476BD65FF40DC83F6831F502826AFEEC3116F71A7A2020CCD254F4BA28FCDC0F96BA2E5264AE9E" },
            new() { Col="c_date", SqlType="DATE", CekGroup=1, Param=P(SqlDbType.Date, new DateTime(2024,3,15)), Label="date", Form="8f460b", Ct="0188B4F75A1F4BDA53C9CDDC1918C09CB57F68E13F5560F1F1D7168FE70707337B1156A97915B244F3C03D3E7352882A599511BD243471FD03683F371CF44E4B76" },

            // Group 2 (CEK CBFB…) — scalar, ciphertext byte-match
            new() { Col="c_dec", SqlType="DECIMAL(28,4)", CekGroup=2, Param=P(SqlDbType.Decimal, 12345.6789m, scale:4, precision:28), Label="decimal(12345.6789)", Form="0115cd5b07000000000000000000000000", Ct="018FAE46024B9B406C23600E6A9C694F9A9B39B785A995689EBE19437BA7E75768011A035A5B54B5E495512EBB46AE1146130940A0D0D834D61AA89B5AD9F71FFAF6EEEAE77E4856BA2AA5E016E2950A8D" },
            new() { Col="c_money", SqlType="MONEY", CekGroup=2, Param=P(SqlDbType.Money, 12.34m), Label="money(12.34)", Form="0000000008e20100", Ct="01B4CE4CAD8D6B241A1555C377A0ADD4C79424DD5162F710D116594F725C1BAB015169A0C7716076EEC90E013519B961DEF427BFC32462D9E45D166C791B73F793" },
            new() { Col="c_smallmoney", SqlType="SMALLMONEY", CekGroup=2, Param=P(SqlDbType.SmallMoney, 12.34m), Label="smallmoney(12.34)", Form="0000000008e20100" },

            // Temporal — normalized-form check (offline harness can't reach these)
            new() { Col="c_time7", SqlType="TIME(7)", CekGroup=0, Param=P(SqlDbType.Time, ts, scale:7), Label="time(7)", Form="07c4aaf46e" },
            new() { Col="c_time3", SqlType="TIME(3)", CekGroup=0, Param=P(SqlDbType.Time, ts, scale:3), Label="time(3)", Form="30b2aaf46e" },
            new() { Col="c_dt2_7", SqlType="DATETIME2(7)", CekGroup=0, Param=P(SqlDbType.DateTime2, dt, scale:7), Label="datetime2(7)", Form="07c4aaf46e8f460b" },
            new() { Col="c_dt2_3", SqlType="DATETIME2(3)", CekGroup=0, Param=P(SqlDbType.DateTime2, dt, scale:3), Label="datetime2(3)", Form="30b2aaf46e8f460b" },
            new() { Col="c_dto7", SqlType="DATETIMEOFFSET(7)", CekGroup=0, Param=P(SqlDbType.DateTimeOffset, dto, scale:7), Label="datetimeoffset(7)", Form="0788f2da408f460b4a01" },
            new() { Col="c_dto3", SqlType="DATETIMEOFFSET(3)", CekGroup=0, Param=P(SqlDbType.DateTimeOffset, dto, scale:3), Label="datetimeoffset(3)", Form="3076f2da408f460b4a01" },
            new() { Col="c_datetime", SqlType="DATETIME", CekGroup=0, Param=P(SqlDbType.DateTime, dtLegacy), Label="datetime(legacy)", Form="34b10000d925da00" },
            new() { Col="c_smalldt", SqlType="SMALLDATETIME", CekGroup=0, Param=P(SqlDbType.SmallDateTime, sdt), Label="smalldatetime", Form="34b11a03" },

            // Fixed-width — normalized-form check (unpadded value bytes)
            new() { Col="c_char", SqlType="CHAR(10)", Collate=" COLLATE Latin1_General_BIN2", CekGroup=0, Param=P(SqlDbType.Char, "Hello", size:10), Label="char(10) Hello", Form="48656c6c6f" },
            new() { Col="c_nchar", SqlType="NCHAR(10)", Collate=" COLLATE Latin1_General_BIN2", CekGroup=0, Param=P(SqlDbType.NChar, "Hello", size:10), Label="nchar(10) Hello", Form="480065006c006c006f00" },
            new() { Col="c_binary", SqlType="BINARY(10)", CekGroup=0, Param=P(SqlDbType.Binary, new byte[]{1,2,3,4,5}, size:10), Label="binary(10)", Form="0102030405" },
        };

        string baseConn = $"Server={server};Database=master;User Id={user};Password={password};TrustServerCertificate=true;Encrypt=true";
        string aeConn = baseConn + ";Column Encryption Setting=Enabled";
        string sfx = Guid.NewGuid().ToString("N").Substring(0, 8);
        string[] cmk = { $"AE86_cmk0_{sfx}", $"AE86_cmk1_{sfx}", $"AE86_cmk2_{sfx}" };
        string[] cek = { $"AE86_cek0_{sfx}", $"AE86_cek1_{sfx}", $"AE86_cek2_{sfx}" };
        string tbl = $"AE86_tbl_{sfx}";
        bool ok = true;

        try
        {
            using (var admin = new SqlConnection(baseConn))
            {
                admin.Open();
                for (int g = 0; g < 3; g++)
                {
                    Exec(admin, $"CREATE COLUMN MASTER KEY [{cmk[g]}] WITH (KEY_STORE_PROVIDER_NAME='IN_MEMORY_KEY_STORE', KEY_PATH='ae86-live-{g}')");
                    Exec(admin, $"CREATE COLUMN ENCRYPTION KEY [{cek[g]}] WITH VALUES (COLUMN_MASTER_KEY=[{cmk[g]}], ALGORITHM='RSA_OAEP', ENCRYPTED_VALUE=0x0102030405060708)");
                }
                var cols = new List<string>();
                foreach (var c in cases)
                    cols.Add($"[{c.Col}] {c.SqlType}{c.Collate} ENCRYPTED WITH (COLUMN_ENCRYPTION_KEY=[{cek[c.CekGroup]}], ENCRYPTION_TYPE=DETERMINISTIC, ALGORITHM='AEAD_AES_256_CBC_HMAC_SHA_256')");
                Exec(admin, $"CREATE TABLE [{tbl}] ({string.Join(", ", cols)})");
            }

            // One provider resolves all three CEKs by the CMK key path it receives,
            // so the whole row inserts under the correct per-group CEK at once.
            using (var conn = new SqlConnection(aeConn))
            {
                conn.Open();
                conn.RegisterColumnEncryptionKeyStoreProvidersOnConnection(new Dictionary<string, SqlColumnEncryptionKeyStoreProvider>
                {
                    ["IN_MEMORY_KEY_STORE"] = new StubKeyStoreProvider(new Dictionary<string, byte[]>
                    {
                        ["ae86-live-0"] = Hex(ceks[0]),
                        ["ae86-live-1"] = Hex(ceks[1]),
                        ["ae86-live-2"] = Hex(ceks[2]),
                    }),
                });
                var colNames = new List<string>();
                var paramNames = new List<string>();
                using var cmd = conn.CreateCommand();
                int n = 0;
                foreach (var c in cases)
                {
                    string pname = $"@p{n++}";
                    colNames.Add($"[{c.Col}]");
                    paramNames.Add(pname);
                    var p = c.Param; p.ParameterName = pname;
                    cmd.Parameters.Add(p);
                }
                cmd.CommandText = $"INSERT INTO [{tbl}] ({string.Join(",", colNames)}) VALUES ({string.Join(",", paramNames)})";
                cmd.ExecuteNonQuery();
            }

            // Read every column's ciphertext back over a plain (non-AE) connection.
            var ct = new Dictionary<string, byte[]>();
            using (var conn = new SqlConnection(baseConn))
            {
                conn.Open();
                using var cmd = conn.CreateCommand();
                var sel = new List<string>();
                foreach (var c in cases) sel.Add($"[{c.Col}]");
                cmd.CommandText = $"SELECT {string.Join(",", sel)} FROM [{tbl}]";
                using var r = cmd.ExecuteReader();
                r.Read();
                for (int i = 0; i < cases.Count; i++) ct[cases[i].Col] = (byte[])r[i];
            }

            foreach (var c in cases)
            {
                byte[] raw = ct[c.Col];
                string liveForm = ToHex(decryptors[c.CekGroup](raw));
                bool formMatch = liveForm == c.Form;
                string ctNote;
                if (c.Ct.Length > 0)
                {
                    bool ctMatch = ToHex(raw) == c.Ct.ToLowerInvariant();
                    ctNote = $"| ciphertext {(ctMatch ? "MATCH" : "DRIFT")}";
                    ok &= ctMatch;
                }
                else ctNote = "| (form only)";
                ok &= formMatch;
                Console.WriteLine($"[live] {c.Label,-22} form {(formMatch ? "MATCH" : $"DRIFT got={liveForm} want={c.Form}")} {ctNote}");
            }
        }
        finally
        {
            try
            {
                using var admin = new SqlConnection(baseConn);
                admin.Open();
                Exec(admin, $"DROP TABLE IF EXISTS [{tbl}]");
                for (int g = 0; g < 3; g++)
                {
                    Exec(admin, $"DROP COLUMN ENCRYPTION KEY IF EXISTS [{cek[g]}]");
                    Exec(admin, $"DROP COLUMN MASTER KEY IF EXISTS [{cmk[g]}]");
                }
            }
            catch { /* idempotent teardown */ }
        }

        Console.WriteLine(ok
            ? "\nPASS: live Microsoft.Data.SqlClient normalizer reproduces every committed fixture"
            : "\nFAIL: drift between the live normalizer and the committed fixtures");
        return ok ? 0 : 1;
    }
}
