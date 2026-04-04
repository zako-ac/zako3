sed -i 's/use std::time::Duration;/use std::time::Duration;\nuse std::fs::File;\nuse std::io::BufReader;\nuse std::path::Path;/' transport/server/src/lib.rs

cat << 'INNER_EOF' >> transport/server/src/lib.rs

pub fn load_certs_and_key<P: AsRef<Path>>(
    cert_path: P,
    key_path: P,
) -> std::io::Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    let cert_file = File::open(cert_path)?;
    let mut cert_reader = BufReader::new(cert_file);
    let cert_chain = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()?;

    let key_file = File::open(key_path)?;
    let mut key_reader = BufReader::new(key_file);
    let private_key = rustls_pemfile::private_key(&mut key_reader)?
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "No private key found in file"))?;

    Ok((cert_chain, private_key))
}
INNER_EOF
