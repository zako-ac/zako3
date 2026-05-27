//! Integration test for the scope-match flow.
//!
//! Spins up a Postgres + pgvector container, runs HQ's migrations, seeds an
//! `EmojiMappingRule` at guild scope, then serves an "almost identical" image
//! and a "totally different" image from a local axum server. Drives
//! `handle_scope_match` directly (no NATS) and asserts the new rule is appended
//! when the image matches, and skipped when it doesn't.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{Router, routing::get};
use hq_types::hq::settings::{EmojiMappingRule, PartialUserSettings, UserSettingsField};
use image::{ImageBuffer, Rgb};
use sqlx::PgPool;
use testcontainers::{
    GenericImage, ImageExt,
    core::{ContainerPort, WaitFor},
    runners::AsyncRunner,
};
use tokio::net::TcpListener;
use zako3_emoji_matcher::config::AppConfig;
use zako3_emoji_matcher::handlers::scope_match::{ScopeMatchContext, handle_scope_match};
use zako3_emoji_matcher::store::{PgHashCache, PgSettingsStore};
use zako3_emoji_matcher_proto::EmojiScopeMatchRequest;

/// Build a deterministic 32x32 image with a horizontal-gradient pattern.
/// `shift` shifts the pattern by N pixels — small shifts produce nearly the
/// same perceptual hash; large shifts produce very different hashes.
fn render_test_image(shift: u32) -> Vec<u8> {
    let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(32, 32);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let v = ((x.wrapping_add(shift) * 8) ^ (y * 4)) as u8;
        *pixel = Rgb([v, v.wrapping_add(40), v.wrapping_add(80)]);
    }
    let mut bytes: Vec<u8> = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("encode");
    bytes
}

/// Build a completely different image — flat color, nothing in common with the
/// gradient images above.
fn render_different_image() -> Vec<u8> {
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_fn(32, 32, |_, _| Rgb([200, 50, 50]));
    let mut bytes: Vec<u8> = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .expect("encode");
    bytes
}

async fn spawn_image_server() -> SocketAddr {
    let app = Router::new()
        .route(
            "/seed.png",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "image/png")],
                    render_test_image(0),
                )
            }),
        )
        .route(
            "/near.png",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "image/png")],
                    render_test_image(1),
                )
            }),
        )
        .route(
            "/different.png",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "image/png")],
                    render_different_image(),
                )
            }),
        );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

async fn seed_guild_rule(pool: &PgPool, guild_id: &str, image_url: String) {
    let rule = EmojiMappingRule {
        emoji_id: "111".to_string(),
        emoji_name: "check".to_string(),
        emoji_image_url: image_url,
        replacement: "Check".to_string(),
    };
    let settings = PartialUserSettings {
        emoji_mappings: UserSettingsField::Normal(vec![rule]),
        ..PartialUserSettings::empty()
    };
    let json = serde_json::to_value(&settings).unwrap();
    sqlx::query(
        "INSERT INTO guild_settings (guild_id, settings) VALUES ($1, $2) \
         ON CONFLICT (guild_id) DO UPDATE SET settings = EXCLUDED.settings",
    )
    .bind(guild_id)
    .bind(json)
    .execute(pool)
    .await
    .unwrap();
}

async fn read_guild_rules(pool: &PgPool, guild_id: &str) -> Vec<EmojiMappingRule> {
    let row: Option<(serde_json::Value,)> =
        sqlx::query_as("SELECT settings FROM guild_settings WHERE guild_id = $1")
            .bind(guild_id)
            .fetch_optional(pool)
            .await
            .unwrap();
    let Some((v,)) = row else { return Vec::new() };
    let settings: PartialUserSettings = serde_json::from_value(v).unwrap();
    match settings.emoji_mappings {
        UserSettingsField::None => Vec::new(),
        UserSettingsField::Normal(v) | UserSettingsField::Important(v) => v,
    }
}

#[tokio::test]
async fn scope_match_writes_new_rule_for_near_duplicate() {
    let container = GenericImage::new("pgvector/pgvector", "pg16")
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_DB", "postgres")
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .with_mapped_port(0, ContainerPort::Tcp(5432))
        .start()
        .await
        .expect("start postgres");

    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = PgPool::connect(&database_url).await.unwrap();

    hq_core::run_migrations(&pool).await.unwrap();

    let server_addr = spawn_image_server().await;
    let seed_url = format!("http://{server_addr}/seed.png");
    let near_url = format!("http://{server_addr}/near.png");
    let diff_url = format!("http://{server_addr}/different.png");

    let guild_id = "g1";
    seed_guild_rule(&pool, guild_id, seed_url).await;

    let cfg = Arc::new(AppConfig {
        nats_url: String::new(),
        http_addr: String::new(),
        otlp_endpoint: None,
        database_url: database_url.clone(),
        worker_concurrency: 1,
        match_hamming_threshold: 4,
        queue_capacity: 16,
    });
    let hash_cache = Arc::new(PgHashCache::new(pool.clone()).await.unwrap());
    let settings_store = Arc::new(PgSettingsStore::new(pool.clone()));
    let ctx = Arc::new(ScopeMatchContext {
        config: cfg,
        hash_cache,
        settings: settings_store,
    });

    // Override the image URL for the seeded rule's emoji_id so the worker
    // hashes the local image instead of trying to hit Discord's CDN.
    // The seed rule's image_url already points at the local server.

    // Case 1: near-duplicate → expect a new rule for emoji_id "222".
    let req = EmojiScopeMatchRequest {
        emoji_id: "222".to_string(),
        emoji_name: "check_v2".to_string(),
        emoji_animated: false,
        guild_id: guild_id.to_string(),
        user_id: None,
    };

    // Trick: the worker builds the URL from the CDN. For the test we monkey-
    // patch by inserting a hash_cache row for "222" computed off our local
    // image, so the worker's `hash_or_cache` path returns it without HTTP.
    let near_bytes = reqwest::get(&near_url).await.unwrap().bytes().await.unwrap();
    let near_img = image::load_from_memory(&near_bytes).unwrap();
    let near_hash = imagehash::perceptual_hash(&near_img);
    ctx.hash_cache
        .put("222", &near_hash.into())
        .await
        .unwrap();

    handle_scope_match(req, ctx.clone()).await.unwrap();

    let rules = read_guild_rules(&pool, guild_id).await;
    assert!(
        rules.iter().any(|r| r.emoji_id == "222" && r.replacement == "Check"),
        "expected new rule for 222 mapped to 'Check', got {rules:?}"
    );

    // Case 2: totally different image → expect no new rule.
    let diff_bytes = reqwest::get(&diff_url).await.unwrap().bytes().await.unwrap();
    let diff_img = image::load_from_memory(&diff_bytes).unwrap();
    let diff_hash = imagehash::perceptual_hash(&diff_img);
    ctx.hash_cache.put("333", &diff_hash.into()).await.unwrap();

    let req2 = EmojiScopeMatchRequest {
        emoji_id: "333".to_string(),
        emoji_name: "unrelated".to_string(),
        emoji_animated: false,
        guild_id: guild_id.to_string(),
        user_id: None,
    };
    handle_scope_match(req2, ctx.clone()).await.unwrap();

    let rules = read_guild_rules(&pool, guild_id).await;
    assert!(
        !rules.iter().any(|r| r.emoji_id == "333"),
        "expected no rule for 333 (image is dissimilar), got {rules:?}"
    );
}
