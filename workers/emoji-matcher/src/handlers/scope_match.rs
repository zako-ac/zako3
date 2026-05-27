use std::sync::Arc;

use hq_types::hq::settings::{EmojiMappingRule, PartialUserSettings, UserSettingsField};
use zako3_emoji_matcher_proto::EmojiScopeMatchRequest;

use crate::config::AppConfig;
use crate::metrics;
use crate::store::{ArcHashCache, ArcSettingsStore};
use crate::types::{ImgHash, Scope};
use crate::utils::image::hash_image;

pub struct ScopeMatchContext {
    pub config: Arc<AppConfig>,
    pub hash_cache: ArcHashCache,
    pub settings: ArcSettingsStore,
}

pub fn cdn_url(emoji_id: &str, animated: bool) -> String {
    let ext = if animated { "gif" } else { "png" };
    format!("https://cdn.discordapp.com/emojis/{emoji_id}.{ext}")
}

async fn hash_or_cache(
    cache: &ArcHashCache,
    emoji_id: &str,
    url: &str,
) -> anyhow::Result<ImgHash> {
    if let Some(h) = cache.get(emoji_id).await? {
        return Ok(h);
    }
    let raw = hash_image(url).await?;
    let h: ImgHash = raw.into();
    if let Err(e) = cache.put(emoji_id, &h).await {
        tracing::warn!(emoji_id, error = %e, "failed to cache emoji hash");
    }
    Ok(h)
}

fn emoji_rules(settings: &PartialUserSettings) -> &[EmojiMappingRule] {
    match &settings.emoji_mappings {
        UserSettingsField::None => &[],
        UserSettingsField::Normal(v) | UserSettingsField::Important(v) => v.as_slice(),
    }
}

pub async fn handle_scope_match(
    request: EmojiScopeMatchRequest,
    ctx: Arc<ScopeMatchContext>,
) -> anyhow::Result<()> {
    metrics::EMOJI_SCOPE_MATCH_REQUESTS.inc();

    let new_url = cdn_url(&request.emoji_id, request.emoji_animated);
    let new_hash = hash_or_cache(&ctx.hash_cache, &request.emoji_id, &new_url).await?;

    // Order: most specific first so we prefer specific scopes when the same
    // rule exists at multiple levels. We still scan all scopes since the user
    // may have configured a mapping only at a less-specific level.
    let mut scopes: Vec<Scope> = Vec::with_capacity(4);
    if let Some(uid) = request.user_id.as_ref() {
        scopes.push(Scope::GuildUser {
            user_id: uid.clone(),
            guild_id: request.guild_id.clone(),
        });
        scopes.push(Scope::User(uid.clone()));
    }
    scopes.push(Scope::Guild(request.guild_id.clone()));
    scopes.push(Scope::Global);

    let threshold = ctx.config.match_hamming_threshold;

    let mut best: Option<(Scope, EmojiMappingRule, u32)> = None;

    for scope in scopes {
        let Some(settings) = ctx.settings.read_scope(&scope).await? else {
            continue;
        };

        for rule in emoji_rules(&settings) {
            if rule.emoji_id == request.emoji_id {
                tracing::debug!(
                    emoji_id = %request.emoji_id,
                    "emoji already mapped in scope; skipping"
                );
                return Ok(());
            }

            let rule_hash =
                match hash_or_cache(&ctx.hash_cache, &rule.emoji_id, &rule.emoji_image_url).await {
                    Ok(h) => h,
                    Err(e) => {
                        tracing::warn!(
                            rule_emoji_id = %rule.emoji_id,
                            error = %e,
                            "failed to hash existing rule image"
                        );
                        continue;
                    }
                };

            let dist = new_hash.hamming(&rule_hash);
            if dist <= threshold {
                let prefer = match &best {
                    None => true,
                    Some((s, _, d)) => {
                        scope.specificity() > s.specificity()
                            || (scope.specificity() == s.specificity() && dist < *d)
                    }
                };
                if prefer {
                    best = Some((scope.clone(), rule.clone(), dist));
                }
            }
        }
    }

    let Some((scope, matched_rule, dist)) = best else {
        tracing::info!(
            emoji_id = %request.emoji_id,
            "no match found across scopes"
        );
        return Ok(());
    };

    tracing::info!(
        emoji_id = %request.emoji_id,
        matched_emoji_id = %matched_rule.emoji_id,
        ?scope,
        dist,
        "scope match — writing new emoji mapping"
    );

    let new_rule = EmojiMappingRule {
        emoji_id: request.emoji_id,
        emoji_name: request.emoji_name,
        emoji_image_url: new_url,
        replacement: matched_rule.replacement,
    };

    ctx.settings.append_emoji_rule(&scope, new_rule).await?;
    metrics::EMOJI_SCOPE_MATCH_HITS.inc();

    Ok(())
}
