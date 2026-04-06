use protofish2::compression::CompressionType;
use protofish2::connection::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use crate::error::{Result, ZakofishError};

pub fn load_certs<P: AsRef<Path>>(path: P) -> Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path.as_ref())
        .map_err(|e| ZakofishError::ProtocolError(format!("Failed to open cert file: {}", e)))?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| ZakofishError::ProtocolError(format!("Failed to parse certs: {}", e)))?;
    Ok(certs)
}

pub fn load_private_key<P: AsRef<Path>>(path: P) -> Result<PrivateKeyDer<'static>> {
    let file = File::open(path.as_ref())
        .map_err(|e| ZakofishError::ProtocolError(format!("Failed to open key file: {}", e)))?;
    let mut reader = BufReader::new(file);
    let key = rustls_pemfile::private_key(&mut reader)
        .map_err(|e| ZakofishError::ProtocolError(format!("Failed to parse key: {}", e)))?
        .ok_or_else(|| ZakofishError::ProtocolError("No private key found in file".to_string()))?;
    Ok(key)
}

/// Creates a ServerConfig with good defaults and full compression support.
pub fn create_server_config<P1: AsRef<Path>, P2: AsRef<Path>>(
    bind_address: SocketAddr,
    cert_file_path: P1,
    key_file_path: P2,
) -> Result<ServerConfig> {
    let cert_chain = load_certs(cert_file_path)?;
    let private_key = load_private_key(key_file_path)?;

    Ok(ServerConfig {
        bind_address,
        cert_chain,
        private_key,
        supported_compression_types: vec![
            CompressionType::Zstd,
            CompressionType::Lz4,
            CompressionType::Gzip,
            CompressionType::None,
        ],
        keepalive_interval: Duration::from_secs(15),
        protofish_config: Default::default(),
    })
}
