trait IdCandidate: Send + Sync {
    /// Tries to get the ID of StatefulSet in k8s
    fn try_get_id(&self) -> Option<u64>;
}

struct PodNameEnv;
struct Hostname;

impl IdCandidate for PodNameEnv {
    fn try_get_id(&self) -> Option<u64> {
        std::env::var("POD_NAME")
            .ok()
            .and_then(|pod_name| pod_name.split('-').last()?.parse::<u64>().ok())
    }
}

impl IdCandidate for Hostname {
    fn try_get_id(&self) -> Option<u64> {
        hostname::get()
            .ok()
            .and_then(|hostname| hostname.to_str()?.split('-').last()?.parse::<u64>().ok())
    }
}

pub fn load_id() -> Option<u64> {
    let candidates: Vec<Box<dyn IdCandidate>> = vec![Box::new(PodNameEnv), Box::new(Hostname)];

    for candidate in candidates {
        if let Some(id) = candidate.try_get_id() {
            return Some(id);
        }
    }
    None
}

pub fn load_discord_token(id: Option<u64>, token_input: String) -> Result<String, String> {
    if token_input.contains(",") {
        if let Some(id) = id {
            let tokens = token_input.split(',').map(|s| s.trim()).collect::<Vec<_>>();
            if let Some(token) = tokens.get(id as usize) {
                Ok(token.to_string())
            } else {
                Err(format!("No token found for ID {} in the provided list", id))
            }
        } else {
            Err("Failed to load ID, but given multiple tokens.".to_string())
        }
    } else {
        Ok(token_input)
    }
}
