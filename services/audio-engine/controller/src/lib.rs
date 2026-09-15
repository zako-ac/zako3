pub mod address;
pub mod config;
pub mod hq_registry;
pub mod ready_waiter;
pub mod server;

/// Serialises the tests that mutate process-wide environment variables.
///
/// `HOSTNAME` and `POD_NAME` belong to the whole test binary, so the address
/// heuristic test and the token-pool test would otherwise overwrite each
/// other's value mid-assertion — which is exactly how
/// `pool_is_indexed_by_pod_ordinal` came to see ordinal 0.
#[cfg(test)]
pub(crate) mod test_env {
    use std::sync::Mutex;

    pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());
}
