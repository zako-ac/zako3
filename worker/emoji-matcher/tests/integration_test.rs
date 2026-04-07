use testcontainers::{
    GenericImage, ImageExt,
    core::{ContainerPort, WaitFor},
    runners::AsyncRunner,
};
use zako3_emoji_matcher::store::{EmojiStore, PgEmojiStore};
use zako3_emoji_matcher::types::{Emoji, ImgHash};

#[tokio::test]
async fn test_save_and_find_emoji() {
    // Use pgvector image
    let image = GenericImage::new("pgvector/pgvector", "pg16")
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_DB", "postgres")
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres");

    let container = image
        .with_mapped_port(0, ContainerPort::Tcp(5432))
        .start()
        .await
        .expect("Failed to start postgres container");

    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");
    let connection_string = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let store = PgEmojiStore::new(&connection_string)
        .await
        .expect("Failed to create store");

    // Create dummy hash (64 bits)
    let mut hash_vec = vec![0.0f32; 64];
    // Set some bits to 1
    hash_vec[0] = 1.0;
    hash_vec[1] = 1.0;
    let hash = ImgHash::from_float_vec(hash_vec);

    let emoji = Emoji {
        hash: hash.clone(),
        registered_id: "test_id".to_string(),
        name: "test_emoji".to_string(),
    };

    store.save_emoji(emoji).await.expect("Failed to save emoji");

    // Search with exact hash
    let found = store
        .find_similar_emoji(&hash)
        .await
        .expect("Failed to find emoji");
    assert!(found.is_some());
    let found_emoji = found.unwrap();
    assert_eq!(found_emoji.registered_id, "test_id");
    assert_eq!(found_emoji.name, "test_emoji");

    // Search with slightly different hash (Hamming distance 1)
    let mut different_hash_vec = hash.to_float_vec();
    different_hash_vec[2] = 1.0; // Flip one bit (0 -> 1)
    let different_hash = ImgHash::from_float_vec(different_hash_vec);

    let found_diff = store
        .find_similar_emoji(&different_hash)
        .await
        .expect("Failed to find similar emoji");
    assert!(found_diff.is_some());
    assert_eq!(found_diff.unwrap().registered_id, "test_id");

    // Search with very different hash (Hamming distance > 5)
    let mut very_different_hash_vec = vec![0.0f32; 64];
    // Set completely different bits
    for i in 10..20 {
        very_different_hash_vec[i] = 1.0;
    }
    let very_different_hash = ImgHash::from_float_vec(very_different_hash_vec);

    let found_none = store
        .find_similar_emoji(&very_different_hash)
        .await
        .expect("Failed to search emoji");
    assert!(found_none.is_none());
}
