use protofish2::Timestamp;
use rcgen::generate_simple_self_signed;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use zako3_taphub_transport_client::TransportClient;
use zako3_taphub_transport_server::{TapHubBridgeHandler, TransportServer};
use zako3_types::{
    AudioCachePolicy, AudioCacheType, AudioMetaResponse, AudioMetadata, AudioRequest,
    CachedAudioRequest,
};

struct MockHandler;

#[async_trait::async_trait]
impl TapHubBridgeHandler for MockHandler {
    async fn handle_request_audio(
        &self,
        req: CachedAudioRequest,
    ) -> Result<(AudioMetaResponse, mpsc::Receiver<(Timestamp, bytes::Bytes)>), String> {
        let (tx, rx) = mpsc::channel(10);

        let meta = AudioMetaResponse {
            cache_key: req.cache_key.clone(),
            metadatas: vec![
                AudioMetadata::Title("Test Title".to_string()),
                AudioMetadata::Artist("Test Artist".to_string()),
            ],
            base_volume: 1.0,
        };

        tokio::spawn(async move {
            let _ = tx;
        });

        Ok((meta, rx))
    }

    async fn handle_preload_audio(
        &self,
        req: CachedAudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        Ok(AudioMetaResponse {
            cache_key: req.cache_key.clone(),
            metadatas: vec![
                AudioMetadata::Title("Preload Title".to_string()),
                AudioMetadata::Artist("Preload Artist".to_string()),
            ],
            base_volume: 1.0,
        })
    }

    async fn handle_request_audio_meta(
        &self,
        _req: AudioRequest,
    ) -> Result<AudioMetaResponse, String> {
        Ok(AudioMetaResponse {
            cache_key: AudioCachePolicy {
                cache_type: AudioCacheType::CacheKey("meta_key".to_string()),
                ttl_seconds: None,
            },
            metadatas: vec![
                AudioMetadata::Title("Meta Title".to_string()),
                AudioMetadata::Artist("Meta Artist".to_string()),
            ],
            base_volume: 1.0,
        })
    }
}

fn generate_certs() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    let subject_alt_names = vec!["localhost".to_string()];
    let cert = generate_simple_self_signed(subject_alt_names).unwrap();
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let key_der = PrivateKeyDer::try_from(cert.key_pair.serialize_der()).unwrap();
    (vec![cert_der], key_der)
}

#[tokio::test]
async fn test_transport_integration() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let (server_certs, server_key) = generate_certs();
    let client_certs = server_certs.clone();

    let server_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

    let mut server =
        TransportServer::new(server_addr, server_certs, server_key, Arc::new(MockHandler))
            .expect("Failed to create server");

    let bound_addr = server.local_addr().expect("Failed to get local address");

    tokio::spawn(async move {
        server.run().await;
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let client_bind_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let client = TransportClient::new(
        client_bind_addr,
        bound_addr,
        "localhost".to_string(),
        client_certs,
    )
    .expect("Failed to create client");

    client.connect().await.expect("Failed to connect client");

    let req = CachedAudioRequest {
        tap_name: "test_tap".to_string().into(),
        audio_request: "yt:preload".to_string().into(),
        cache_key: AudioCachePolicy {
            cache_type: AudioCacheType::CacheKey("preload_key".to_string()),
            ttl_seconds: None,
        },
        discord_user_id: "123".to_string().into(),
    };

    let resp = client
        .preload_audio(req)
        .await
        .expect("preload_audio failed");
    if let AudioCacheType::CacheKey(k) = resp.cache_key.cache_type {
        assert_eq!(k, "preload_key");
    } else {
        panic!("Expected CacheKey");
    }

    // Check title in metadatas
    let mut found_title = false;
    for m in resp.metadatas {
        if let AudioMetadata::Title(t) = m {
            assert_eq!(t, "Preload Title");
            found_title = true;
        }
    }
    assert!(found_title);

    let meta_req = AudioRequest {
        tap_name: "test_tap".to_string().into(),
        request: "yt:meta".to_string().into(),
        discord_user_id: "123".to_string().into(),
    };
    let meta_resp = client
        .request_audio_meta(meta_req)
        .await
        .expect("request_audio_meta failed");
    if let AudioCacheType::CacheKey(k) = meta_resp.cache_key.cache_type {
        assert_eq!(k, "meta_key");
    } else {
        panic!("Expected CacheKey");
    }
}
