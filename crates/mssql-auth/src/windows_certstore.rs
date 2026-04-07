//! Windows Certificate Store Column Master Key (CMK) provider for Always Encrypted.
//!
//! This module provides integration with the Windows Certificate Store for secure key
//! management in SQL Server Always Encrypted scenarios.
//!
//! ## Overview
//!
//! The Windows Certificate Store is the operating system's built-in secure storage
//! for certificates and their associated private keys. This provider uses the Windows
//! CNG (Cryptography Next Generation) APIs to perform cryptographic operations.
//!
//! ## CMK Path Format
//!
//! The CMK path for Windows Certificate Store follows this format:
//!
//! ```text
//! CurrentUser/My/<thumbprint>
//! LocalMachine/My/<thumbprint>
//! ```
//!
//! Where:
//! - `CurrentUser` or `LocalMachine` specifies the store location
//! - `My` is the store name (typically "My" for personal certificates)
//! - `<thumbprint>` is the certificate's SHA-1 thumbprint in hex format
//!
//! ## Security Considerations
//!
//! - Private keys never leave the Windows CNG key storage
//! - Access is controlled via Windows ACLs on the private key
//! - Hardware keys (TPM, smart cards) are supported transparently
//! - All operations use the Windows CNG API, not the legacy CryptoAPI
//!
//! ## Example
//!
//! ```rust,ignore
//! use mssql_auth::windows_certstore::WindowsCertStoreProvider;
//! use mssql_auth::ColumnEncryptionConfig;
//!
//! // Create provider
//! let provider = WindowsCertStoreProvider::new();
//!
//! // Register with encryption config
//! let config = ColumnEncryptionConfig::new()
//!     .with_provider(provider);
//! ```
//!
//! ## Platform Requirements
//!
//! This module is only available on Windows and requires the `windows-certstore` feature.

use std::ffi::c_void;

use tracing::{debug, instrument};
use windows::Win32::Foundation::BOOL;
use windows::Win32::Security::Cryptography::CryptAcquireCertificatePrivateKey;
use windows::Win32::Security::Cryptography::{
    BCRYPT_OAEP_PADDING_INFO,
    // Constants
    BCRYPT_PAD_OAEP,
    BCRYPT_PAD_PKCS1,
    BCRYPT_PKCS1_PADDING_INFO,
    CERT_CLOSE_STORE_CHECK_FLAG,
    CERT_FIND_HASH,
    CERT_OPEN_STORE_FLAGS,
    CERT_QUERY_ENCODING_TYPE,
    CERT_STORE_PROV_SYSTEM_W,
    CRYPT_ACQUIRE_ONLY_NCRYPT_KEY_FLAG,
    CRYPT_HASH_BLOB,
    // Certificate store functions
    CertCloseStore,
    CertFindCertificateInStore,
    CertFreeCertificateContext,
    CertOpenStore,
    NCRYPT_FLAGS,
    NCRYPT_KEY_HANDLE,
    NCRYPT_SILENT_FLAG,
    // CNG functions
    NCryptDecrypt,
    NCryptFreeObject,
    NCryptSignHash,
    NCryptVerifySignature,
    X509_ASN_ENCODING,
};
use windows::core::PCWSTR;

use crate::encryption::{EncryptionError, KeyStoreProvider};

/// SQL Server provider name for Windows Certificate Store.
const PROVIDER_NAME: &str = "MSSQL_CERTIFICATE_STORE";

/// Windows Certificate Store Column Master Key provider.
///
/// This provider implements the [`KeyStoreProvider`] trait to support
/// Always Encrypted operations using certificates stored in the Windows
/// Certificate Store.
///
/// ## Thread Safety
///
/// This provider is `Send + Sync` and can be safely shared across threads.
/// However, the underlying Windows CNG handles are managed per-operation.
#[derive(Debug, Clone, Default)]
pub struct WindowsCertStoreProvider {
    _private: (),
}

impl WindowsCertStoreProvider {
    /// Create a new Windows Certificate Store provider.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let provider = WindowsCertStoreProvider::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Parse a CMK path into store location, store name, and thumbprint.
    ///
    /// Expected format: `<StoreLocation>/<StoreName>/<Thumbprint>`
    ///
    /// Examples:
    /// - `CurrentUser/My/ABC123...`
    /// - `LocalMachine/My/DEF456...`
    fn parse_cmk_path(cmk_path: &str) -> Result<(StoreLocation, String, Vec<u8>), EncryptionError> {
        let parts: Vec<&str> = cmk_path.split('/').collect();

        if parts.len() < 3 {
            return Err(EncryptionError::CmkError(format!(
                "Invalid CMK path format: expected '<StoreLocation>/<StoreName>/<Thumbprint>', got '{}'",
                cmk_path
            )));
        }

        let store_location = match parts[0].to_uppercase().as_str() {
            "CURRENTUSER" | "CURRENT_USER" => StoreLocation::CurrentUser,
            "LOCALMACHINE" | "LOCAL_MACHINE" => StoreLocation::LocalMachine,
            _ => {
                return Err(EncryptionError::CmkError(format!(
                    "Unknown store location: '{}'. Expected 'CurrentUser' or 'LocalMachine'",
                    parts[0]
                )));
            }
        };

        let store_name = parts[1].to_string();

        // Parse thumbprint (hex string)
        let thumbprint_hex = parts[2..].join("");
        let thumbprint = hex_to_bytes(&thumbprint_hex)
            .map_err(|e| EncryptionError::CmkError(format!("Invalid thumbprint hex: {}", e)))?;

        Ok((store_location, store_name, thumbprint))
    }

    /// Get a certificate's private key handle from the Windows Certificate Store.
    fn get_private_key(
        store_location: StoreLocation,
        store_name: &str,
        thumbprint: &[u8],
    ) -> Result<CngKeyHandle, EncryptionError> {
        // Open the certificate store
        let store_name_wide: Vec<u16> = store_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: CertOpenStore is called with a valid null-terminated UTF-16 store name
        // (store_name_wide, created above). The returned handle is checked for errors and
        // wrapped in CertStoreGuard for RAII cleanup via CertCloseStore.
        let store = unsafe {
            CertOpenStore(
                CERT_STORE_PROV_SYSTEM_W,
                CERT_QUERY_ENCODING_TYPE(0),
                None,
                CERT_OPEN_STORE_FLAGS(store_location.to_flags()),
                Some(store_name_wide.as_ptr() as *const c_void),
            )
        }
        .map_err(|e| {
            EncryptionError::CmkError(format!(
                "Failed to open certificate store '{}': {}",
                store_name, e
            ))
        })?;

        // Create RAII wrapper for store
        let store_guard = CertStoreGuard(store);

        // Create hash blob for certificate lookup
        let hash_blob = CRYPT_HASH_BLOB {
            cbData: thumbprint.len() as u32,
            pbData: thumbprint.as_ptr() as *mut u8,
        };

        // Find the certificate by thumbprint
        // SAFETY: store_guard.0 is a valid store handle (from CertOpenStore above).
        // hash_blob points to the stack-allocated thumbprint slice which outlives this call.
        // The returned pointer is checked for null before use and wrapped in CertContextGuard.
        let cert_context = unsafe {
            CertFindCertificateInStore(
                store_guard.0,
                X509_ASN_ENCODING,
                0,
                CERT_FIND_HASH,
                Some(&hash_blob as *const _ as *const c_void),
                None,
            )
        };

        if cert_context.is_null() {
            return Err(EncryptionError::CmkError(format!(
                "Certificate not found with thumbprint: {}",
                bytes_to_hex(thumbprint)
            )));
        }

        // Create RAII wrapper for certificate context
        let cert_guard = CertContextGuard(cert_context);

        // Acquire the private key
        let mut key_handle = NCRYPT_KEY_HANDLE::default();
        let mut key_spec = 0u32;
        let mut caller_free = BOOL::from(false);

        // SAFETY: cert_guard.0 is a valid certificate context (from CertFindCertificateInStore).
        // Output parameters (key_handle, key_spec, caller_free) are stack-allocated mutable
        // references. The result is checked before using key_handle, which is then wrapped
        // in CngKeyHandle for RAII cleanup via NCryptFreeObject.
        let result = unsafe {
            CryptAcquireCertificatePrivateKey(
                cert_guard.0,
                CRYPT_ACQUIRE_ONLY_NCRYPT_KEY_FLAG,
                None,
                &mut key_handle,
                Some(&mut key_spec),
                Some(&mut caller_free),
            )
        };

        if result.is_err() {
            return Err(EncryptionError::CmkError(format!(
                "Failed to acquire private key for certificate: {:?}",
                result.err()
            )));
        }

        Ok(CngKeyHandle {
            handle: key_handle,
            should_free: caller_free.as_bool(),
        })
    }
}

#[async_trait::async_trait]
impl KeyStoreProvider for WindowsCertStoreProvider {
    fn provider_name(&self) -> &str {
        PROVIDER_NAME
    }

    #[instrument(skip(self, encrypted_cek), fields(cmk_path = %cmk_path, algorithm = %algorithm))]
    async fn decrypt_cek(
        &self,
        cmk_path: &str,
        algorithm: &str,
        encrypted_cek: &[u8],
    ) -> Result<Vec<u8>, EncryptionError> {
        debug!("Decrypting CEK using Windows Certificate Store");

        // Parse the CMK path
        let (store_location, store_name, thumbprint) = Self::parse_cmk_path(cmk_path)?;

        // Get the private key handle
        let key_handle = Self::get_private_key(store_location, &store_name, &thumbprint)?;

        // Parse the SQL Server encrypted CEK format
        let ciphertext = parse_sql_server_encrypted_cek(encrypted_cek)?;

        // Determine padding based on algorithm
        let (padding_info, flags) = get_padding_info(algorithm)?;

        // First call to get required output size
        let mut result_size = 0u32;
        // SAFETY: key_handle.handle is valid (from CryptAcquireCertificatePrivateKey).
        // ciphertext is a valid &[u8] slice. padding_info.as_ptr() points to the stack-
        // allocated PaddingInfo enum. Output buffer is None (size query only). The result
        // is checked before using result_size.
        let decrypt_result = unsafe {
            NCryptDecrypt(
                key_handle.handle,
                Some(ciphertext),
                Some(padding_info.as_ptr()),
                None,
                &mut result_size,
                flags,
            )
        };

        if decrypt_result.is_err() {
            return Err(EncryptionError::CekDecryptionFailed(format!(
                "NCryptDecrypt (size query) failed: {:?}",
                decrypt_result.err()
            )));
        }

        // Allocate buffer and perform actual decryption
        let mut output = vec![0u8; result_size as usize];
        // SAFETY: Same preconditions as the size query above. output is a freshly allocated
        // Vec with capacity from the size query. result_size is updated with actual bytes
        // written, and output is truncated to this size to prevent reading uninitialized memory.
        let decrypt_result = unsafe {
            NCryptDecrypt(
                key_handle.handle,
                Some(ciphertext),
                Some(padding_info.as_ptr()),
                Some(&mut output),
                &mut result_size,
                flags,
            )
        };

        if decrypt_result.is_err() {
            return Err(EncryptionError::CekDecryptionFailed(format!(
                "NCryptDecrypt failed: {:?}",
                decrypt_result.err()
            )));
        }

        output.truncate(result_size as usize);
        debug!("Successfully decrypted CEK using Windows Certificate Store");
        Ok(output)
    }

    #[instrument(skip(self, data), fields(cmk_path = %cmk_path))]
    async fn sign_data(&self, cmk_path: &str, data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        debug!("Signing data using Windows Certificate Store");

        // Parse the CMK path
        let (store_location, store_name, thumbprint) = Self::parse_cmk_path(cmk_path)?;

        // Get the private key handle
        let key_handle = Self::get_private_key(store_location, &store_name, &thumbprint)?;

        // Use PKCS#1 v1.5 padding with SHA-256 for signing
        let hash_algorithm: Vec<u16> = "SHA256\0".encode_utf16().collect();
        let padding_info = BCRYPT_PKCS1_PADDING_INFO {
            pszAlgId: PCWSTR(hash_algorithm.as_ptr()),
        };

        // First call to get required signature size
        let mut sig_size = 0u32;
        // SAFETY: key_handle.handle is valid. padding_info is a stack-allocated
        // BCRYPT_PKCS1_PADDING_INFO with pszAlgId pointing to hash_algorithm (valid for
        // the duration of this call). data is a valid &[u8]. Output buffer is None
        // (size query). Result is checked before using sig_size.
        let sign_result = unsafe {
            NCryptSignHash(
                key_handle.handle,
                Some(&padding_info as *const _ as *const c_void),
                data,
                None,
                &mut sig_size,
                BCRYPT_PAD_PKCS1,
            )
        };

        if sign_result.is_err() {
            return Err(EncryptionError::CmkError(format!(
                "NCryptSignHash (size query) failed: {:?}",
                sign_result.err()
            )));
        }

        // Allocate buffer and perform actual signing
        let mut signature = vec![0u8; sig_size as usize];
        // SAFETY: Same preconditions as the size query above. signature is a freshly
        // allocated Vec with capacity from the size query. sig_size is updated with actual
        // bytes written, and signature is truncated to this size.
        let sign_result = unsafe {
            NCryptSignHash(
                key_handle.handle,
                Some(&padding_info as *const _ as *const c_void),
                data,
                Some(&mut signature),
                &mut sig_size,
                BCRYPT_PAD_PKCS1,
            )
        };

        if sign_result.is_err() {
            return Err(EncryptionError::CmkError(format!(
                "NCryptSignHash failed: {:?}",
                sign_result.err()
            )));
        }

        signature.truncate(sig_size as usize);
        debug!("Successfully signed data using Windows Certificate Store");
        Ok(signature)
    }

    #[instrument(skip(self, data, signature), fields(cmk_path = %cmk_path))]
    async fn verify_signature(
        &self,
        cmk_path: &str,
        data: &[u8],
        signature: &[u8],
    ) -> Result<bool, EncryptionError> {
        debug!("Verifying signature using Windows Certificate Store");

        // Parse the CMK path
        let (store_location, store_name, thumbprint) = Self::parse_cmk_path(cmk_path)?;

        // Get the private key handle (we'll use it for verification too)
        let key_handle = Self::get_private_key(store_location, &store_name, &thumbprint)?;

        // Use PKCS#1 v1.5 padding with SHA-256 for verification
        let hash_algorithm: Vec<u16> = "SHA256\0".encode_utf16().collect();
        let padding_info = BCRYPT_PKCS1_PADDING_INFO {
            pszAlgId: PCWSTR(hash_algorithm.as_ptr()),
        };

        // Perform verification
        // SAFETY: key_handle.handle is valid. padding_info is a stack-allocated
        // BCRYPT_PKCS1_PADDING_INFO. data and signature are valid &[u8] slices.
        // This is a read-only verification operation with no output buffers.
        let verify_result = unsafe {
            NCryptVerifySignature(
                key_handle.handle,
                Some(&padding_info as *const _ as *const c_void),
                data,
                signature,
                BCRYPT_PAD_PKCS1,
            )
        };

        let is_valid = verify_result.is_ok();
        debug!("Signature verification result: {}", is_valid);
        Ok(is_valid)
    }
}

/// Certificate store location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoreLocation {
    CurrentUser,
    LocalMachine,
}

impl StoreLocation {
    /// Convert to Windows store flags.
    fn to_flags(self) -> u32 {
        match self {
            StoreLocation::CurrentUser => 0x00010000, // CERT_SYSTEM_STORE_CURRENT_USER
            StoreLocation::LocalMachine => 0x00020000, // CERT_SYSTEM_STORE_LOCAL_MACHINE
        }
    }
}

/// RAII wrapper for certificate store handle.
struct CertStoreGuard(windows::Win32::Security::Cryptography::HCERTSTORE);

impl Drop for CertStoreGuard {
    fn drop(&mut self) {
        // SAFETY: self.0 was obtained from a successful CertOpenStore call. Drop is
        // called at most once (guaranteed by the type system). The return value is
        // intentionally ignored as cleanup errors during drop should not panic.
        let _ = unsafe { CertCloseStore(self.0, CERT_CLOSE_STORE_CHECK_FLAG) };
    }
}

/// RAII wrapper for certificate context.
struct CertContextGuard(*const windows::Win32::Security::Cryptography::CERT_CONTEXT);

impl Drop for CertContextGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: self.0 is checked for null before freeing. If non-null, it was
            // obtained from CertFindCertificateInStore. Drop is called at most once.
            unsafe { CertFreeCertificateContext(Some(self.0)) };
        }
    }
}

/// RAII wrapper for CNG key handle.
struct CngKeyHandle {
    handle: NCRYPT_KEY_HANDLE,
    should_free: bool,
}

impl Drop for CngKeyHandle {
    fn drop(&mut self) {
        if self.should_free && !self.handle.is_invalid() {
            // SAFETY: Freeing is guarded by should_free (set by CryptAcquireCertificatePrivateKey's
            // caller_free output) and is_invalid() (checks handle validity). Drop is called at
            // most once. The return value is intentionally ignored during cleanup.
            let _ = unsafe { NCryptFreeObject(self.handle.0 as _) };
        }
    }
}

/// Padding info wrapper that can hold either OAEP or PKCS1 padding.
enum PaddingInfo {
    Oaep(BCRYPT_OAEP_PADDING_INFO),
    #[allow(dead_code)]
    Pkcs1(BCRYPT_PKCS1_PADDING_INFO),
}

impl PaddingInfo {
    fn as_ptr(&self) -> *const c_void {
        match self {
            PaddingInfo::Oaep(info) => info as *const _ as *const c_void,
            PaddingInfo::Pkcs1(info) => info as *const _ as *const c_void,
        }
    }
}

/// Get padding info based on algorithm name.
fn get_padding_info(algorithm: &str) -> Result<(PaddingInfo, NCRYPT_FLAGS), EncryptionError> {
    // SHA-256 hash algorithm string (null-terminated UTF-16)
    static SHA256_ALG: &str = "SHA256\0";

    match algorithm.to_uppercase().as_str() {
        "RSA_OAEP" | "RSA-OAEP" | "RSA_OAEP_256" | "RSA-OAEP-256" => {
            let hash_alg: Vec<u16> = SHA256_ALG.encode_utf16().collect();
            // SAFETY: Box::leak is used intentionally to produce a 'static PCWSTR pointer.
            // BCRYPT_OAEP_PADDING_INFO requires a valid PCWSTR for its lifetime, but the
            // PaddingInfo is returned to the caller and may outlive any local borrow.
            // The leak is bounded: at most 2 allocations (one per algorithm branch) per
            // decrypt_cek call. This is acceptable because decrypt_cek is called infrequently
            // (only during CEK decryption for Always Encrypted column access).
            let hash_alg_ptr = Box::leak(hash_alg.into_boxed_slice());

            let info = BCRYPT_OAEP_PADDING_INFO {
                pszAlgId: PCWSTR(hash_alg_ptr.as_ptr()),
                pbLabel: std::ptr::null_mut(),
                cbLabel: 0,
            };
            Ok((
                PaddingInfo::Oaep(info),
                NCRYPT_FLAGS(BCRYPT_PAD_OAEP.0 | NCRYPT_SILENT_FLAG.0),
            ))
        }
        "RSA1_5" | "RSA-1_5" | "RSA_PKCS1" | "RSA-PKCS1" => {
            let hash_alg: Vec<u16> = SHA256_ALG.encode_utf16().collect();
            // SAFETY: Box::leak is used intentionally to produce a 'static PCWSTR pointer.
            // BCRYPT_PKCS1_PADDING_INFO requires a valid PCWSTR for its lifetime. See the
            // RSA_OAEP branch above for the full rationale. The leak is bounded to at most
            // 2 allocations per decrypt_cek call.
            let hash_alg_ptr = Box::leak(hash_alg.into_boxed_slice());

            let info = BCRYPT_PKCS1_PADDING_INFO {
                pszAlgId: PCWSTR(hash_alg_ptr.as_ptr()),
            };
            Ok((
                PaddingInfo::Pkcs1(info),
                NCRYPT_FLAGS(BCRYPT_PAD_PKCS1.0 | NCRYPT_SILENT_FLAG.0),
            ))
        }
        _ => Err(EncryptionError::ConfigurationError(format!(
            "Unsupported key encryption algorithm: {}. Expected RSA_OAEP, RSA_OAEP_256, or RSA1_5",
            algorithm
        ))),
    }
}

/// Parse the SQL Server encrypted CEK format to extract the raw ciphertext.
///
/// SQL Server CEK format:
/// - Version (1 byte): 0x01
/// - Key path length (2 bytes, LE)
/// - Key path (UTF-16LE)
/// - Ciphertext length (2 bytes, LE)
/// - Ciphertext (RSA encrypted CEK)
fn parse_sql_server_encrypted_cek(data: &[u8]) -> Result<&[u8], EncryptionError> {
    if data.len() < 5 {
        return Err(EncryptionError::CekDecryptionFailed(
            "Encrypted CEK too short".into(),
        ));
    }

    // Check version byte
    if data[0] != 0x01 {
        return Err(EncryptionError::CekDecryptionFailed(format!(
            "Invalid CEK version: expected 0x01, got {:#04x}",
            data[0]
        )));
    }

    // Read key path length (2 bytes, little-endian)
    let key_path_len = u16::from_le_bytes([data[1], data[2]]) as usize;

    // Calculate offset after key path
    let ciphertext_len_offset = 3 + key_path_len;
    if data.len() < ciphertext_len_offset + 2 {
        return Err(EncryptionError::CekDecryptionFailed(
            "Encrypted CEK truncated: missing ciphertext length".into(),
        ));
    }

    // Read ciphertext length (2 bytes, little-endian)
    let ciphertext_len =
        u16::from_le_bytes([data[ciphertext_len_offset], data[ciphertext_len_offset + 1]]) as usize;

    // Calculate ciphertext offset
    let ciphertext_offset = ciphertext_len_offset + 2;
    if data.len() < ciphertext_offset + ciphertext_len {
        return Err(EncryptionError::CekDecryptionFailed(format!(
            "Encrypted CEK truncated: expected {} bytes of ciphertext, got {}",
            ciphertext_len,
            data.len() - ciphertext_offset
        )));
    }

    Ok(&data[ciphertext_offset..ciphertext_offset + ciphertext_len])
}

/// Convert a hex string to bytes.
fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, &'static str> {
    let hex = hex.trim();
    if hex.len() % 2 != 0 {
        return Err("Hex string has odd length");
    }

    hex.as_bytes()
        .chunks(2)
        .map(|chunk| {
            let high = char::from(chunk[0])
                .to_digit(16)
                .ok_or("Invalid hex digit")?;
            let low = char::from(chunk[1])
                .to_digit(16)
                .ok_or("Invalid hex digit")?;
            Ok((high * 16 + low) as u8)
        })
        .collect()
}

/// Convert bytes to hex string.
fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02X}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cmk_path() {
        // Valid path with CurrentUser
        let (location, name, thumb) =
            WindowsCertStoreProvider::parse_cmk_path("CurrentUser/My/AABBCCDD").unwrap();
        assert_eq!(location, StoreLocation::CurrentUser);
        assert_eq!(name, "My");
        assert_eq!(thumb, vec![0xAA, 0xBB, 0xCC, 0xDD]);

        // Valid path with LocalMachine (case insensitive)
        let (location, name, _) =
            WindowsCertStoreProvider::parse_cmk_path("localmachine/My/1234").unwrap();
        assert_eq!(location, StoreLocation::LocalMachine);
        assert_eq!(name, "My");

        // Valid path with underscores
        let (location, _, _) =
            WindowsCertStoreProvider::parse_cmk_path("Current_User/My/1234").unwrap();
        assert_eq!(location, StoreLocation::CurrentUser);
    }

    #[test]
    fn test_parse_cmk_path_invalid() {
        // Missing thumbprint
        assert!(WindowsCertStoreProvider::parse_cmk_path("CurrentUser/My").is_err());

        // Invalid location
        assert!(WindowsCertStoreProvider::parse_cmk_path("Invalid/My/1234").is_err());

        // Invalid hex
        assert!(WindowsCertStoreProvider::parse_cmk_path("CurrentUser/My/GGGG").is_err());
    }

    #[test]
    fn test_hex_conversion() {
        assert_eq!(
            hex_to_bytes("AABBCCDD").unwrap(),
            vec![0xAA, 0xBB, 0xCC, 0xDD]
        );
        assert_eq!(
            hex_to_bytes("aabbccdd").unwrap(),
            vec![0xAA, 0xBB, 0xCC, 0xDD]
        );
        assert_eq!(hex_to_bytes("").unwrap(), vec![]);
        assert!(hex_to_bytes("ABC").is_err()); // Odd length
        assert!(hex_to_bytes("GGGG").is_err()); // Invalid chars
    }

    #[test]
    fn test_bytes_to_hex() {
        assert_eq!(bytes_to_hex(&[0xAA, 0xBB, 0xCC, 0xDD]), "AABBCCDD");
        assert_eq!(bytes_to_hex(&[0x01, 0x02, 0x0F]), "01020F");
        assert_eq!(bytes_to_hex(&[]), "");
    }

    #[test]
    fn test_parse_sql_server_encrypted_cek() {
        // Create a valid encrypted CEK structure
        let key_path = "test";
        let key_path_utf16: Vec<u8> = key_path
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        let ciphertext = vec![0xAB, 0xCD, 0xEF];

        let mut data = Vec::new();
        data.push(0x01); // Version
        data.extend_from_slice(&(key_path_utf16.len() as u16).to_le_bytes());
        data.extend_from_slice(&key_path_utf16);
        data.extend_from_slice(&(ciphertext.len() as u16).to_le_bytes());
        data.extend_from_slice(&ciphertext);

        let parsed = parse_sql_server_encrypted_cek(&data).unwrap();
        assert_eq!(parsed, &ciphertext[..]);
    }

    #[test]
    fn test_parse_sql_server_encrypted_cek_invalid() {
        // Too short
        assert!(parse_sql_server_encrypted_cek(&[0x01, 0x00]).is_err());

        // Wrong version
        assert!(parse_sql_server_encrypted_cek(&[0x02, 0x00, 0x00, 0x00, 0x00]).is_err());
    }

    #[test]
    fn test_store_location_flags() {
        assert_eq!(StoreLocation::CurrentUser.to_flags(), 0x00010000);
        assert_eq!(StoreLocation::LocalMachine.to_flags(), 0x00020000);
    }
}
