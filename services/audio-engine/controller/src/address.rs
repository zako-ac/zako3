/// Trait for resolving the address that this AE should advertise to TL.
pub trait SelfAddressResolver: Send + Sync {
    /// Resolve and return the advertised address in the form "host:port"
    fn resolve(&self) -> Result<String, String>;
}

/// Heuristic-based address resolver for Kubernetes StatefulSet deployments.
///
/// Logic:
/// - If "audio-engine" appears in the hostname, advertise the hostname directly
/// - Otherwise, resolve the machine's outbound IP and advertise that
pub struct HeuristicSelfAddressResolver {
    port: u16,
}

impl HeuristicSelfAddressResolver {
    pub fn new(port: u16) -> Self {
        Self { port }
    }
}

impl SelfAddressResolver for HeuristicSelfAddressResolver {
    fn resolve(&self) -> Result<String, String> {
        // Try to get hostname from environment or gethostname
        let hostname = std::env::var("HOSTNAME")
            .or_else(|_| {
                gethostname::gethostname()
                    .into_string()
                    .map_err(|_| "Failed to get hostname".to_string())
            })
            .map_err(|e| format!("Failed to resolve hostname: {}", e))?;

        if hostname.contains("audio-engine") {
            // Kubernetes StatefulSet mode: use the hostname directly
            Ok(format!("http://{}:{}", hostname, self.port))
        } else {
            // Fallback: resolve outbound IP by connecting to an external socket
            resolve_outbound_ip(self.port)
        }
    }
}

/// Resolve the machine's outbound IP by connecting a UDP socket to an external address.
/// This avoids returning localhost or non-routable addresses.
fn resolve_outbound_ip(port: u16) -> Result<String, String> {
    use std::net::SocketAddr;
    use std::net::UdpSocket;

    // Connect to a public DNS server (doesn't actually send data, just determines routing)
    let target: SocketAddr = "8.8.8.8:80"
        .parse()
        .map_err(|e| format!("Parse error: {}", e))?;

    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Bind error: {}", e))?;

    socket
        .connect(target)
        .map_err(|e| format!("Connect error: {}", e))?;

    socket
        .local_addr()
        .map(|addr| format!("http://{}:{}", addr.ip(), port))
        .map_err(|e| format!("Local addr error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic_with_audio_engine_hostname() {
        // Simulate setting an audio-engine hostname
        unsafe {
            std::env::set_var("HOSTNAME", "audio-engine-0");
        }
        let resolver = HeuristicSelfAddressResolver::new(8090);
        let result = resolver.resolve();
        assert!(result.is_ok());
        assert!(result.unwrap().contains("audio-engine-0"));
    }
}
