use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;

use sha2::Digest;
use tracing::{warn, error};

use crate::{
    model::MapperSummary,
    repo::MapperRepository,
    service::ProcessContext,
    wasm::{
        input::{CallerInfo, MappingList, MapperListPayload, WasmInput},
        runner, EngineState,
    },
    Result,
};

pub(crate) async fn execute_pipeline(
    ordered_ids: Vec<String>,
    ctx: ProcessContext,
    wasm_dir: &Path,
    mapper_repo: &dyn MapperRepository,
    engine_state: Arc<EngineState>,
) -> Result<String> {
    let mut text = ctx.text.clone();
    let mut remaining: VecDeque<String> = VecDeque::from(ordered_ids);
    let mut completed = Vec::new();

    while let Some(mapper_id) = remaining.pop_front() {
        // Load mapper from repo
        let mapper = match mapper_repo.find_by_id(&mapper_id).await {
            Ok(Some(m)) => m,
            Ok(None) => {
                warn!(mapper_id = %mapper_id, "mapper not found in repo, skipping");
                continue;
            }
            Err(e) => {
                error!(mapper_id = %mapper_id, error = %e, "failed to load mapper from repo");
                continue;
            }
        };

        // Load WASM file
        let wasm_path = wasm_dir.join(&mapper.wasm_filename);
        let wasm_bytes = match tokio::fs::read(&wasm_path).await {
            Ok(bytes) => bytes,
            Err(e) => {
                warn!(
                    mapper_id = %mapper_id,
                    path = ?wasm_path,
                    error = %e,
                    "failed to read wasm file, skipping"
                );
                continue;
            }
        };

        // Verify SHA256
        let actual_hash = hex::encode(sha2::Sha256::digest(&wasm_bytes));
        if actual_hash != mapper.sha256_hash {
            error!(
                mapper_id = %mapper_id,
                expected = %mapper.sha256_hash,
                actual = %actual_hash,
                "sha256 mismatch, skipping mapper"
            );
            continue;
        }

        // Build WASM input
        let needs_mapping_list = mapper
            .input_data
            .contains(&crate::model::MapperInputData::MappingList);
        let needs_caller_info = mapper
            .input_data
            .contains(&crate::model::MapperInputData::CallerInfo);
        let needs_mapper_list = mapper
            .input_data
            .contains(&crate::model::MapperInputData::MapperList);
        let needs_discord_info = mapper
            .input_data
            .contains(&crate::model::MapperInputData::DiscordInfo);

        let future_ids: Vec<String> = remaining.iter().cloned().collect();

        let wasm_input = WasmInput {
            text: &text,
            mapping_list: if needs_mapping_list {
                Some(MappingList {
                    text_rules: &ctx.text_mappings,
                    emoji_rules: &ctx.emoji_mappings,
                })
            } else {
                None
            },
            caller_info: if needs_caller_info {
                Some(CallerInfo {
                    user_id: ctx.caller.0.clone(),
                })
            } else {
                None
            },
            mapper_list: if needs_mapper_list {
                Some(MapperListPayload {
                    previous: &completed,
                    future: &future_ids,
                })
            } else {
                None
            },
            guild_id: if needs_discord_info {
                Some(ctx.guild_id.into())
            } else {
                None
            },
            channel_id: if needs_discord_info {
                Some(ctx.channel_id.into())
            } else {
                None
            },
        };

        let stdin_json = serde_json::to_vec(&wasm_input)?;

        // Run WASM in spawn_blocking
        let engine = engine_state.engine.clone();
        let rt_handle = tokio::runtime::Handle::current();
        let discord_info = ctx.discord_info.clone();
        let guild_id = ctx.guild_id;
        let mapper_name = mapper.name.clone();

        let output = tokio::task::spawn_blocking(move || {
            runner::run_mapper_sync(
                &engine,
                &wasm_bytes,
                stdin_json,
                needs_discord_info,
                rt_handle,
                discord_info,
                guild_id,
            )
        })
        .await;

        match output {
            Ok(Ok(out)) => {
                if let Some(ref err) = out.error {
                    warn!(
                        mapper_id = %mapper_id,
                        error = %err,
                        "mapper reported error"
                    );
                }

                // Update text if output is non-empty
                if !out.text.is_empty() {
                    text = out.text;
                }

                // Handle override
                if let Some(override_ids) = out.override_future_mappers {
                    remaining = VecDeque::from(override_ids);
                }

                completed.push(MapperSummary {
                    id: mapper_id,
                    name: mapper_name,
                    success: out.error.is_none(),
                });
            }
            Ok(Err(e)) => {
                warn!(
                    mapper_id = %mapper_id,
                    error = %e,
                    "mapper execution failed, skipping"
                );
                completed.push(MapperSummary {
                    id: mapper_id,
                    name: mapper_name,
                    success: false,
                });
            }
            Err(e) => {
                warn!(
                    mapper_id = %mapper_id,
                    error = %e,
                    "spawn_blocking join error"
                );
                completed.push(MapperSummary {
                    id: mapper_id,
                    name: mapper_name,
                    success: false,
                });
            }
        }
    }

    Ok(text)
}
