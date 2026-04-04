sed -i 's/use std::time::Duration;/use std::time::Duration;\nuse std::fs::File;\nuse std::io::BufReader;\nuse std::path::Path;/' transport/client/src/lib.rs

cat << 'INNER_EOF' >> transport/client/src/lib.rs

pub fn load_certs<P: AsRef<Path>>(path: P) -> std::io::Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(certs)
}
INNER_EOF
