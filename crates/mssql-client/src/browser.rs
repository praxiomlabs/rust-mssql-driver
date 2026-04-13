//! SQL Server Browser service client for named instance resolution.
//!
//! When connecting to a named SQL Server instance (e.g., `localhost\SQLEXPRESS`),
//! the client needs to discover which TCP port the instance is listening on.
//! The SQL Server Browser service runs on UDP port 1434 and answers these queries
//! using the SQL Server Resolution Protocol (SSRP), defined in [MC-SQLR].
//!
//! [MC-SQLR]: https://learn.microsoft.com/en-us/openspecs/windows_protocols/mc-sqlr/
//!
//! # Protocol
//!
//! The client sends a `CLNT_UCAST_INST` message (0x04 + instance name + null byte)
//! and receives a `SVR_RESP` message (0x05 + length + semicolon-delimited data).
//! The response contains key-value pairs including `tcp;PORT` for the TCP port.
//!
//! # Example
//!
//! ```rust,ignore
//! use mssql_client::browser::resolve_instance;
//! use std::time::Duration;
//!
//! let port = resolve_instance("localhost", "SQLEXPRESS", Duration::from_secs(2)).await?;
//! println!("SQLEXPRESS is on port {}", port);
//! ```

use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::time::timeout;

use crate::error::Error;

/// Default timeout for SQL Browser queries.
const DEFAULT_BROWSER_TIMEOUT: Duration = Duration::from_secs(2);

/// UDP port for the SQL Server Browser service.
const BROWSER_PORT: u16 = 1434;

/// CLNT_UCAST_INST message type byte.
const CLNT_UCAST_INST: u8 = 0x04;

/// SVR_RESP message type byte.
const SVR_RESP: u8 = 0x05;

/// Maximum response size per the MC-SQLR spec (1024 bytes for CLNT_UCAST_INST).
const MAX_RESPONSE_SIZE: usize = 1024 + 3; // 3 bytes for header (type + length)

/// Resolve a named SQL Server instance to its TCP port via the SQL Browser service.
///
/// Sends a `CLNT_UCAST_INST` query to the SQL Server Browser service running
/// on `host:1434` and parses the response to extract the TCP port number.
///
/// # Arguments
///
/// * `host` - The hostname or IP address of the SQL Server machine
/// * `instance` - The instance name (e.g., "SQLEXPRESS")
/// * `browser_timeout` - How long to wait for the Browser service to respond
///
/// # Errors
///
/// Returns an error if:
/// - The Browser service doesn't respond within the timeout
/// - The instance name is not found in the response
/// - No TCP port is configured for the instance
/// - The response is malformed
///
/// # Example
///
/// ```rust,ignore
/// let port = resolve_instance("localhost", "SQLEXPRESS", Duration::from_secs(2)).await?;
/// ```
pub(crate) async fn resolve_instance(
    host: &str,
    instance: &str,
    browser_timeout: Option<Duration>,
) -> Result<u16, Error> {
    let timeout_duration = browser_timeout.unwrap_or(DEFAULT_BROWSER_TIMEOUT);

    // Normalize host: "." means localhost
    let resolved_host = if host == "." { "127.0.0.1" } else { host };

    let target = format!("{resolved_host}:{BROWSER_PORT}");

    tracing::debug!(
        host = resolved_host,
        instance = instance,
        "querying SQL Browser service at {}",
        target
    );

    // Build the CLNT_UCAST_INST request: 0x04 + instance name (ASCII) + 0x00
    let mut request = Vec::with_capacity(2 + instance.len());
    request.push(CLNT_UCAST_INST);
    request.extend_from_slice(instance.as_bytes());
    request.push(0x00); // null terminator

    // Bind to any available local port for the UDP socket
    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| Error::BrowserResolution {
            instance: instance.to_string(),
            reason: format!("failed to bind UDP socket: {e}"),
        })?;

    // Send the query
    socket
        .send_to(&request, &target)
        .await
        .map_err(|e| Error::BrowserResolution {
            instance: instance.to_string(),
            reason: format!("failed to send to {target}: {e}"),
        })?;

    // Wait for response with timeout
    let mut buf = vec![0u8; MAX_RESPONSE_SIZE];
    let recv_len = timeout(timeout_duration, socket.recv(&mut buf))
        .await
        .map_err(|_| Error::BrowserResolution {
            instance: instance.to_string(),
            reason: format!(
                "SQL Browser service at {target} did not respond within {timeout_duration:?}. \
                 Ensure the SQL Server Browser service is running on the target machine."
            ),
        })?
        .map_err(|e| Error::BrowserResolution {
            instance: instance.to_string(),
            reason: format!("failed to receive from {target}: {e}"),
        })?;

    // Parse the SVR_RESP response
    parse_browser_response(&buf[..recv_len], instance)
}

/// Parse a SVR_RESP message and extract the TCP port for the given instance.
fn parse_browser_response(data: &[u8], instance: &str) -> Result<u16, Error> {
    // Minimum valid response: 0x05 + 2-byte length + at least some data
    if data.len() < 3 {
        return Err(Error::BrowserResolution {
            instance: instance.to_string(),
            reason: "response too short".to_string(),
        });
    }

    if data[0] != SVR_RESP {
        return Err(Error::BrowserResolution {
            instance: instance.to_string(),
            reason: format!(
                "unexpected response type: {:#04x} (expected {SVR_RESP:#04x})",
                data[0]
            ),
        });
    }

    let resp_size = u16::from_le_bytes([data[1], data[2]]) as usize;
    if data.len() < 3 + resp_size {
        return Err(Error::BrowserResolution {
            instance: instance.to_string(),
            reason: format!(
                "response truncated: header says {resp_size} bytes but only {} available",
                data.len() - 3
            ),
        });
    }

    // RESP_DATA is a semicolon-delimited string
    let resp_data =
        std::str::from_utf8(&data[3..3 + resp_size]).map_err(|e| Error::BrowserResolution {
            instance: instance.to_string(),
            reason: format!("response is not valid UTF-8: {e}"),
        })?;

    tracing::debug!(
        instance = instance,
        response = resp_data,
        "SQL Browser response received"
    );

    // Parse semicolon-delimited key-value pairs.
    // Format: ServerName;VALUE;InstanceName;VALUE;IsClustered;VALUE;Version;VALUE;tcp;PORT;;
    let parts: Vec<&str> = resp_data.split(';').collect();

    // Find the "tcp" key and extract the port value (next element)
    let tcp_port = parts
        .windows(2)
        .find(|pair| pair[0].eq_ignore_ascii_case("tcp"))
        .and_then(|pair| pair[1].parse::<u16>().ok());

    match tcp_port {
        Some(port) => {
            tracing::info!(
                instance = instance,
                port = port,
                "resolved named instance via SQL Browser"
            );
            Ok(port)
        }
        None => Err(Error::BrowserResolution {
            instance: instance.to_string(),
            reason: format!(
                "no TCP port found in Browser response. The instance may only support \
                 Named Pipes. Response: {resp_data}"
            ),
        }),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_browser_response_valid() {
        // Build a realistic SVR_RESP
        let resp_data =
            b"ServerName;MYSERVER;InstanceName;SQLEXPRESS;IsClustered;No;Version;15.0.2000.5;tcp;52000;;";
        let mut packet = Vec::new();
        packet.push(SVR_RESP);
        packet.extend_from_slice(&(resp_data.len() as u16).to_le_bytes());
        packet.extend_from_slice(resp_data);

        let port = parse_browser_response(&packet, "SQLEXPRESS").unwrap();
        assert_eq!(port, 52000);
    }

    #[test]
    fn test_parse_browser_response_with_named_pipes() {
        let resp_data = b"ServerName;SRV;InstanceName;INST;IsClustered;No;Version;15.0.0.0;np;\\\\SRV\\pipe\\sql\\query;tcp;49500;;";
        let mut packet = Vec::new();
        packet.push(SVR_RESP);
        packet.extend_from_slice(&(resp_data.len() as u16).to_le_bytes());
        packet.extend_from_slice(resp_data);

        let port = parse_browser_response(&packet, "INST").unwrap();
        assert_eq!(port, 49500);
    }

    #[test]
    fn test_parse_browser_response_no_tcp() {
        let resp_data =
            b"ServerName;SRV;InstanceName;INST;IsClustered;No;Version;15.0.0.0;np;\\\\SRV\\pipe\\sql\\query;;";
        let mut packet = Vec::new();
        packet.push(SVR_RESP);
        packet.extend_from_slice(&(resp_data.len() as u16).to_le_bytes());
        packet.extend_from_slice(resp_data);

        let result = parse_browser_response(&packet, "INST");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("no TCP port"), "got: {err}");
    }

    #[test]
    fn test_parse_browser_response_too_short() {
        let result = parse_browser_response(&[0x05, 0x00], "INST");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_browser_response_wrong_type() {
        let result = parse_browser_response(&[0x04, 0x00, 0x00], "INST");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unexpected response type"), "got: {err}");
    }

    #[test]
    fn test_parse_browser_response_spec_example() {
        // Exact example from MC-SQLR spec
        let packet: Vec<u8> = vec![
            0x05, 0x58, 0x00, // SVR_RESP header (type=0x05, length=0x0058=88)
            // "ServerName;ILSUNG1;InstanceName;YUKONSTD;IsClustered;No;Version;9.00.1399.06;tcp;57137;;"
            0x53, 0x65, 0x72, 0x76, 0x65, 0x72, 0x4e, 0x61, 0x6d, 0x65, 0x3b, 0x49, 0x4c, 0x53,
            0x55, 0x4e, 0x47, 0x31, 0x3b, 0x49, 0x6e, 0x73, 0x74, 0x61, 0x6e, 0x63, 0x65, 0x4e,
            0x61, 0x6d, 0x65, 0x3b, 0x59, 0x55, 0x4b, 0x4f, 0x4e, 0x53, 0x54, 0x44, 0x3b, 0x49,
            0x73, 0x43, 0x6c, 0x75, 0x73, 0x74, 0x65, 0x72, 0x65, 0x64, 0x3b, 0x4e, 0x6f, 0x3b,
            0x56, 0x65, 0x72, 0x73, 0x69, 0x6f, 0x6e, 0x3b, 0x39, 0x2e, 0x30, 0x30, 0x2e, 0x31,
            0x33, 0x39, 0x39, 0x2e, 0x30, 0x36, 0x3b, 0x74, 0x63, 0x70, 0x3b, 0x35, 0x37, 0x31,
            0x33, 0x37, 0x3b, 0x3b,
        ];

        let port = parse_browser_response(&packet, "YUKONSTD").unwrap();
        assert_eq!(port, 57137);
    }
}
