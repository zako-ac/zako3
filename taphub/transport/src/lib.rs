use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct TapHubTransportConfig {
    pub cert_pem: String,
    pub key_pem: String,
    pub listen_addr: String,
}
