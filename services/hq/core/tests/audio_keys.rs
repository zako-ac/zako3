//! Cache-key derivation and the wire↔domain conversions.
//!
//! These are the parts of the ported taphub logic that need no database. They
//! are also the parts whose breakage is silent: a changed key derivation does
//! not error, it just misses every lookup and quietly doubles tap traffic.

use hq_core::service::audio::{arhash_key, build_cache_item, convert_metadata, convert_policy};
use hq_types::cache::AudioCacheItemKey;
use hq_types::hq::audio_dispatch::SinkTicket;
use hq_types::hq::TapId;
use hq_types::{AudioCachePolicy, AudioCacheType, AudioMetadata, AudioRequestString};

fn tap() -> TapId {
    TapId("tap-1".into())
}

fn ars() -> AudioRequestString {
    AudioRequestString("https://example.invalid/song".into())
}

fn policy(t: AudioCacheType, ttl: Option<u32>) -> AudioCachePolicy {
    AudioCachePolicy { cache_type: t, ttl_seconds: ttl }
}

// --- cache keys ------------------------------------------------------------

#[test]
fn an_uncacheable_policy_has_no_item() {
    assert!(build_cache_item(&tap(), &policy(AudioCacheType::None, None), &ars()).is_none());
}

#[test]
fn ar_hash_keys_are_derived_from_the_request_string() {
    let item = build_cache_item(&tap(), &policy(AudioCacheType::ARHash, None), &ars()).unwrap();
    assert_eq!(item.key, arhash_key(&ars()));
    assert_eq!(item.tap_id, tap());
}

#[test]
fn different_requests_hash_to_different_keys() {
    let a = arhash_key(&AudioRequestString("a".into()));
    let b = arhash_key(&AudioRequestString("b".into()));
    assert_ne!(a, b);
}

#[test]
fn the_same_request_always_hashes_the_same_way() {
    assert_eq!(arhash_key(&ars()), arhash_key(&ars()));
}

/// Pinned against the value taphub produces, so the two paths address the same
/// entries while they run side by side. Change this and every existing cached
/// item becomes unreachable.
#[test]
fn ar_hash_is_hex_sha256_of_the_request() {
    use sha2::Digest;
    let expected = hex::encode(sha2::Sha256::digest(ars().to_string().as_bytes()));
    assert_eq!(arhash_key(&ars()), AudioCacheItemKey::ARHash(expected));
}

#[test]
fn an_explicit_cache_key_is_used_verbatim() {
    let item = build_cache_item(
        &tap(),
        &policy(AudioCacheType::CacheKey("track-42".into()), None),
        &ars(),
    )
    .unwrap();
    assert_eq!(item.key, AudioCacheItemKey::CacheKey("track-42".into()));
}

#[test]
fn a_ttl_becomes_an_expiry_and_its_absence_means_forever() {
    let with = build_cache_item(&tap(), &policy(AudioCacheType::ARHash, Some(300)), &ars()).unwrap();
    let without = build_cache_item(&tap(), &policy(AudioCacheType::ARHash, None), &ars()).unwrap();

    let expire_at = with.expire_at.expect("a ttl should set an expiry");
    let delta = (expire_at - chrono::Utc::now()).num_seconds();
    assert!((295..=305).contains(&delta), "expiry was {delta}s away");
    assert!(without.expire_at.is_none());
}

/// Audio filed under a `CacheKey` still gets its metadata filed under the
/// ARHash, so metadata-only lookups hit. The two keys must differ, or the alias
/// would overwrite the audio entry.
#[test]
fn the_metadata_alias_uses_a_different_key_than_the_audio() {
    let audio = build_cache_item(
        &tap(),
        &policy(AudioCacheType::CacheKey("track-42".into()), None),
        &ars(),
    )
    .unwrap();
    assert_ne!(audio.key, arhash_key(&ars()));
}

// --- wire ↔ domain conversions ---------------------------------------------

#[test]
fn every_metadata_variant_maps_to_its_counterpart() {
    use zakofish4_common::model::AudioMetadata as W;
    let cases = vec![
        (W::Title("t".into()), AudioMetadata::Title("t".into())),
        (W::Description("d".into()), AudioMetadata::Description("d".into())),
        (W::Artist("a".into()), AudioMetadata::Artist("a".into())),
        (W::Album("al".into()), AudioMetadata::Album("al".into())),
        (W::ImageUrl("i".into()), AudioMetadata::ImageUrl("i".into())),
        (W::Url("u".into()), AudioMetadata::Url("u".into())),
    ];
    for (wire, domain) in cases {
        assert_eq!(
            format!("{:?}", convert_metadata(wire)),
            format!("{domain:?}")
        );
    }
}

#[test]
fn cache_policies_survive_the_crate_boundary() {
    use zakofish4_common::model::{AudioCachePolicy as WP, AudioCacheType as WT};

    let none = convert_policy(WP { cache_type: WT::None, ttl_seconds: None });
    assert!(matches!(none.cache_type, AudioCacheType::None));
    assert!(none.ttl_seconds.is_none());

    let hash = convert_policy(WP { cache_type: WT::ARHash, ttl_seconds: Some(60) });
    assert!(matches!(hash.cache_type, AudioCacheType::ARHash));
    assert_eq!(hash.ttl_seconds, Some(60));

    let keyed = convert_policy(WP {
        cache_type: WT::CacheKey("k".into()),
        ttl_seconds: None,
    });
    match keyed.cache_type {
        AudioCacheType::CacheKey(k) => assert_eq!(k, "k"),
        other => panic!("expected CacheKey, got {other:?}"),
    }
}

// --- sink tickets ----------------------------------------------------------

#[test]
fn a_freshly_minted_ticket_is_accepted() {
    let ticket = SinkTicket {
        request_id: uuid::Uuid::new_v4(),
        encryption_key: [9u8; 32],
    };
    assert!(ticket.looks_valid());
}

/// Catches a caller that forgot to mint. Without the check it would fail much
/// later as an authentication failure on the UDP path, which looks like a
/// network fault rather than a bug in the caller.
#[test]
fn an_unminted_ticket_is_rejected() {
    assert!(!SinkTicket {
        request_id: uuid::Uuid::new_v4(),
        encryption_key: [0u8; 32],
    }
    .looks_valid());

    assert!(!SinkTicket {
        request_id: uuid::Uuid::nil(),
        encryption_key: [9u8; 32],
    }
    .looks_valid());
}

/// The key travels through logging-adjacent code on both sides, and `Debug` is
/// the easiest way for one to end up in a log line.
#[test]
fn a_ticket_does_not_print_its_key() {
    let ticket = SinkTicket {
        request_id: uuid::Uuid::new_v4(),
        encryption_key: [0xAB; 32],
    };
    let shown = format!("{ticket:?}");
    assert!(shown.contains("redacted"), "{shown}");
    assert!(!shown.contains("171"), "{shown}");
}
