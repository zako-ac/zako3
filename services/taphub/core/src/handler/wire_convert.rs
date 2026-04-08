/// Convert a wire-protocol AudioMetadata (from zakofish-tap) to the
/// zako3-types domain AudioMetadata used by the rest of the system.
pub(crate) fn wire_metadata_to_domain(
    m: zakofish_taphub::types::AudioMetadata,
) -> zako3_types::AudioMetadata {
    use zakofish_taphub::types::AudioMetadata as W;
    match m {
        W::Title(s) => zako3_types::AudioMetadata::Title(s),
        W::Description(s) => zako3_types::AudioMetadata::Description(s),
        W::Artist(s) => zako3_types::AudioMetadata::Artist(s),
        W::Album(s) => zako3_types::AudioMetadata::Album(s),
        W::ImageUrl(s) => zako3_types::AudioMetadata::ImageUrl(s),
    }
}

pub(crate) fn wire_metadatas_to_domain(
    ms: Vec<zakofish_taphub::types::AudioMetadata>,
) -> Vec<zako3_types::AudioMetadata> {
    ms.into_iter().map(wire_metadata_to_domain).collect()
}

/// Convert a wire-protocol AudioCachePolicy to the zako3-types domain type.
pub(crate) fn wire_cache_policy_to_domain(
    p: zakofish_taphub::types::AudioCachePolicy,
) -> zako3_types::AudioCachePolicy {
    use zakofish_taphub::types::AudioCacheType as W;
    let cache_type = match p.cache_type {
        W::None => zako3_types::AudioCacheType::None,
        W::ARHash => zako3_types::AudioCacheType::ARHash,
        W::CacheKey(k) => zako3_types::AudioCacheType::CacheKey(k),
    };
    zako3_types::AudioCachePolicy {
        cache_type,
        ttl_seconds: p.ttl_seconds,
    }
}
