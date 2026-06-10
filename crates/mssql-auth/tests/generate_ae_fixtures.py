#!/usr/bin/env python3
"""Generate AEAD_AES_256_CBC_HMAC_SHA256 interop fixtures from the spec.

Implements the Always Encrypted cell-encryption algorithm independently of
the driver, following the .NET reference implementation
(dotnet/SqlClient SqlAeadAes256CbcHmac256{EncryptionKey,Algorithm}.cs):

  derived_key = HMAC-SHA256(CEK, UTF-16LE(salt))
  salt        = "Microsoft SQL Server cell <enc|MAC|IV> key with encryption
                 algorithm:AEAD_AES_256_CBC_HMAC_SHA256 and key length:256"
  cell_blob   = version(0x01) || HMAC-SHA256 tag(32) || IV(16) || AES-256-CBC
  tag         = HMAC-SHA256(mac_key, version || IV || ciphertext || 0x01)
  IV (det.)   = HMAC-SHA256(iv_key, plaintext)[..16]

The emitted constants are pasted into tests/ae_interop.rs. Any driver that
fails to decrypt these blobs cannot decrypt data encrypted by .NET/SSMS/JDBC.

Regenerate with: python3 generate_ae_fixtures.py  (requires `cryptography`)
"""

import hashlib
import hmac

from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

ALGORITHM = "AEAD_AES_256_CBC_HMAC_SHA256"
KEY_LENGTH = 256
VERSION = b"\x01"

CEK = bytes(range(32))  # matches test_cek() in src/aead.rs
PLAINTEXT = "Hello, SQL Server Always Encrypted!".encode("utf-8")


def derive_key(cek: bytes, kind: str) -> bytes:
    salt = (
        f"Microsoft SQL Server cell {kind} key with encryption "
        f"algorithm:{ALGORITHM} and key length:{KEY_LENGTH}"
    )
    return hmac.new(cek, salt.encode("utf-16-le"), hashlib.sha256).digest()


def encrypt_deterministic(cek: bytes, plaintext: bytes) -> bytes:
    enc_key = derive_key(cek, "encryption")
    mac_key = derive_key(cek, "MAC")
    iv_key = derive_key(cek, "IV")

    iv = hmac.new(iv_key, plaintext, hashlib.sha256).digest()[:16]

    pad = 16 - len(plaintext) % 16
    padded = plaintext + bytes([pad]) * pad
    encryptor = Cipher(algorithms.AES(enc_key), modes.CBC(iv)).encryptor()
    ciphertext = encryptor.update(padded) + encryptor.finalize()

    tag = hmac.new(
        mac_key, VERSION + iv + ciphertext + b"\x01", hashlib.sha256
    ).digest()
    return VERSION + tag + iv + ciphertext


def rust_array(name: str, data: bytes) -> str:
    body = ", ".join(f"0x{b:02x}" for b in data)
    return f"const {name}: [u8; {len(data)}] = [{body}];"


if __name__ == "__main__":
    print(rust_array("SPEC_ENC_KEY", derive_key(CEK, "encryption")))
    print(rust_array("SPEC_MAC_KEY", derive_key(CEK, "MAC")))
    print(rust_array("SPEC_IV_KEY", derive_key(CEK, "IV")))
    blob = encrypt_deterministic(CEK, PLAINTEXT)
    print(rust_array("SPEC_DETERMINISTIC_BLOB", blob))
