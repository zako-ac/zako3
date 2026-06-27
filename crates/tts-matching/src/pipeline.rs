use std::collections::VecDeque;
use std::sync::Arc;

use sha2::Digest;
use tracing::{error, warn};

use crate::{
    Result,
    model::{MapperStepResult, MapperSummary},
    repo::MapperRepository,
    service::ProcessContext,
    wasm::{
        EngineState,
        input::{CallerInfo, MappingList, MapperListPayload, WasmInput},
        runner,
    },
};

pub(crate) async fn execute_pipeline(
    ordered_ids: Vec<String>,
    ctx: ProcessContext,
    mapper_repo: &dyn MapperRepository,
    engine_state: Arc<EngineState>,
) -> Result<String> {
    let mut text = ctx.text.clone();
    let mut remaining: VecDeque<String> = VecDeque::from(ordered_ids);
    let mut completed = Vec::new();

    while let Some(mapper_id) = remaining.pop_front() {
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

        let actual_hash = hex::encode(sha2::Sha256::digest(&mapper.wasm_bytes));
        if actual_hash != mapper.sha256_hash {
            error!(
                mapper_id = %mapper_id,
                expected = %mapper.sha256_hash,
                actual = %actual_hash,
                "sha256 mismatch, skipping mapper"
            );
            continue;
        }

        let module = match engine_state.get_or_compile(&actual_hash, &mapper.wasm_bytes) {
            Ok(m) => m,
            Err(e) => {
                error!(
                    mapper_id = %mapper_id,
                    error = %e,
                    "failed to compile wasm module, skipping"
                );
                continue;
            }
        };

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

        let rt_handle = tokio::runtime::Handle::current();
        let discord_info = ctx.discord_info.clone();
        let guild_id = ctx.guild_id;
        let mapper_name = mapper.name.clone();

        let output = tokio::task::spawn_blocking(move || {
            runner::run_mapper_sync(
                &module,
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

                // Always accept the mapper's text output (including empty string,
                // which signals "block TTS"). Only skip the update on error —
                // error outputs carry no meaningful text.
                if out.error.is_none() {
                    text = out.text;
                }

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

/// Like [`execute_pipeline`] but returns per-step text transformation results.
///
/// Skipped mappers (not found, hash mismatch, compile error) are excluded from the returned steps.
pub(crate) async fn execute_pipeline_traced(
    ordered_ids: Vec<String>,
    ctx: ProcessContext,
    mapper_repo: &dyn MapperRepository,
    engine_state: Arc<EngineState>,
) -> Result<(String, Vec<MapperStepResult>)> {
    let mut text = ctx.text.clone();
    let mut remaining: VecDeque<String> = VecDeque::from(ordered_ids);
    let mut completed = Vec::new();
    let mut steps: Vec<MapperStepResult> = Vec::new();

    while let Some(mapper_id) = remaining.pop_front() {
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

        let actual_hash = hex::encode(sha2::Sha256::digest(&mapper.wasm_bytes));
        if actual_hash != mapper.sha256_hash {
            error!(mapper_id = %mapper_id, expected = %mapper.sha256_hash, actual = %actual_hash, "sha256 mismatch, skipping mapper");
            continue;
        }

        let module = match engine_state.get_or_compile(&actual_hash, &mapper.wasm_bytes) {
            Ok(m) => m,
            Err(e) => {
                error!(mapper_id = %mapper_id, error = %e, "failed to compile wasm module, skipping");
                continue;
            }
        };

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
        let input_text = text.clone();

        let rt_handle = tokio::runtime::Handle::current();
        let discord_info = ctx.discord_info.clone();
        let guild_id = ctx.guild_id;
        let mapper_name = mapper.name.clone();

        let output = tokio::task::spawn_blocking(move || {
            runner::run_mapper_sync(
                &module,
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
                let error = out.error.clone();
                if let Some(ref err) = error {
                    warn!(mapper_id = %mapper_id, error = %err, "mapper reported error");
                }

                // Always accept the mapper's text output (including empty string,
                // which signals "block TTS"). Only skip the update on error.
                if out.error.is_none() {
                    text = out.text;
                }
                if let Some(override_ids) = out.override_future_mappers {
                    remaining = VecDeque::from(override_ids);
                }

                let success = error.is_none();
                completed.push(MapperSummary {
                    id: mapper_id.clone(),
                    name: mapper_name.clone(),
                    success,
                });
                steps.push(MapperStepResult {
                    mapper_id,
                    mapper_name,
                    input_text,
                    output_text: text.clone(),
                    success,
                    error,
                });
            }
            Ok(Err(e)) => {
                warn!(mapper_id = %mapper_id, error = %e, "mapper execution failed");
                completed.push(MapperSummary {
                    id: mapper_id.clone(),
                    name: mapper_name.clone(),
                    success: false,
                });
                steps.push(MapperStepResult {
                    mapper_id,
                    mapper_name,
                    input_text: input_text.clone(),
                    output_text: input_text,
                    success: false,
                    error: Some(e.to_string()),
                });
            }
            Err(e) => {
                warn!(mapper_id = %mapper_id, error = %e, "spawn_blocking join error");
                completed.push(MapperSummary {
                    id: mapper_id.clone(),
                    name: mapper_name.clone(),
                    success: false,
                });
                steps.push(MapperStepResult {
                    mapper_id,
                    mapper_name,
                    input_text: input_text.clone(),
                    output_text: input_text,
                    success: false,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    Ok((text, steps))
}
