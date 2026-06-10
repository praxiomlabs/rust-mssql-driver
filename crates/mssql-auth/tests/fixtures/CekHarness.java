// Regeneration/cross-validation harness for the CEK envelope interop fixture
// (`MS_JDBC_ENVELOPE` in ../cek_envelope_interop.rs).
//
// Uses Microsoft's shipped JDBC driver — SQLServerColumnEncryptionJavaKeyStoreProvider
// is the public, cross-platform Always Encrypted key store provider
// (MSSQL_JAVA_KEYSTORE). It produces and consumes the same canonical
// encrypted-CEK envelope as every other Microsoft client (version ||
// keyPathLen || cipherLen || keyPath || ciphertext || RSA-PKCS1-SHA256
// signature, with the signature verified on decrypt).
//
// Full recipe, starting from this directory:
//
//   # 1. Microsoft's shipped binary (Maven Central)
//   curl -sO https://repo1.maven.org/maven2/com/microsoft/sqlserver/mssql-jdbc/12.10.1.jre11/mssql-jdbc-12.10.1.jre11.jar
//
//   # 2. PKCS12 keystore from the committed test key. The self-signed cert is
//   #    just packaging: the envelope depends only on the RSA key, alias, and
//   #    CEK, so regenerating the cert does not invalidate the fixture.
//   openssl req -new -x509 -key test_cmk_rsa2048.pem -out cert.pem -days 2 \
//       -subj "/CN=rust-mssql-driver-cek-test"
//   openssl pkcs12 -export -inkey test_cmk_rsa2048.pem -in cert.pem \
//       -out keystore.p12 -name "rmd-test-cmk" -password pass:testpass
//
//   # 3. Compile and run
//   javac -cp mssql-jdbc-12.10.1.jre11.jar CekHarness.java
//   java -cp .:mssql-jdbc-12.10.1.jre11.jar CekHarness keystore.p12
//
// Outputs ENVELOPE:<hex> (a fresh envelope for the test CEK; OAEP is
// randomized, so bytes differ per run — any run is a valid fixture) and
// ROUNDTRIP:OK. To prove the reverse direction, pass a Rust-built envelope
// hex as the second argument (emit one via
// `cargo test -p mssql-auth --features always-encrypted --test
// cek_envelope_interop -- --nocapture`, the OUR_ENVELOPE line):
//
//   java -cp .:mssql-jdbc-12.10.1.jre11.jar CekHarness keystore.p12 <hex>
//
// DECRYPT_OURS must print 000102...1F — Microsoft's binary parsed our
// layout, verified our signature, and unwrapped our OAEP-SHA1 ciphertext.
import com.microsoft.sqlserver.jdbc.SQLServerColumnEncryptionJavaKeyStoreProvider;

public class CekHarness {
    public static void main(String[] args) throws Exception {
        var provider = new SQLServerColumnEncryptionJavaKeyStoreProvider(args[0], "testpass".toCharArray());
        String alias = "rmd-test-cmk";

        byte[] cek = new byte[32];
        for (int i = 0; i < 32; i++) cek[i] = (byte) i;

        byte[] envelope = provider.encryptColumnEncryptionKey(alias, "RSA_OAEP", cek);
        System.out.println("ENVELOPE:" + hex(envelope));

        byte[] roundtrip = provider.decryptColumnEncryptionKey(alias, "RSA_OAEP", envelope);
        System.out.println("ROUNDTRIP:" + (java.util.Arrays.equals(roundtrip, cek) ? "OK" : "FAIL"));

        if (args.length > 1) {
            byte[] ours = provider.decryptColumnEncryptionKey(alias, "RSA_OAEP", fromHex(args[1]));
            System.out.println("DECRYPT_OURS:" + hex(ours));
        }
    }

    static String hex(byte[] b) {
        StringBuilder s = new StringBuilder();
        for (byte x : b) s.append(String.format("%02X", x));
        return s.toString();
    }

    static byte[] fromHex(String h) {
        byte[] b = new byte[h.length() / 2];
        for (int i = 0; i < b.length; i++)
            b[i] = (byte) Integer.parseInt(h.substring(2 * i, 2 * i + 2), 16);
        return b;
    }
}
