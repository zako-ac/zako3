use std::fs;
use std::path::PathBuf;
use zako3_types::hq::audit_log::*;
use zako3_types::hq::*;
use zod_gen::ZodGenerator;

fn main() {
    let mut generator = ZodGenerator::new();

    generator.add_schema::<CreateTapDto>("CreateTapDto");
    generator.add_schema::<UpdateTapDto>("UpdateTapDto");
    generator.add_schema::<AuthCallbackDto>("AuthCallbackDto");
    generator.add_schema::<AuthUserDto>("AuthUserDto");
    generator.add_schema::<AuthResponseDto>("AuthResponseDto");
    generator.add_schema::<LoginResponseDto>("LoginResponseDto");
    generator.add_schema::<UserSummaryDto>("UserSummaryDto");
    generator.add_schema::<TapDto>("TapDto");
    generator.add_schema::<TapWithAccessDto>("TapWithAccessDto");
    generator.add_schema::<TimeSeriesPointDto>("TimeSeriesPointDto");
    generator.add_schema::<TapStatsDto>("TapStatsDto");
    generator.add_schema::<PaginationMetaDto>("PaginationMetaDto");

    // We handle generics explicitly by creating aliases or just not adding PaginatedResponseDto since TS handles generics better manually.
    // Wait, the generated file won't have PaginatedResponseDto. We can just append it manually to the file.

    generator.add_schema::<CreateApiKeyDto>("CreateApiKeyDto");
    generator.add_schema::<UpdateApiKeyDto>("UpdateApiKeyDto");
    generator.add_schema::<ApiKeyDto>("ApiKeyDto");
    generator.add_schema::<ApiKeyResponseDto>("ApiKeyResponseDto");
    generator.add_schema::<NotificationDto>("NotificationDto");
    generator.add_schema::<CreateNotificationDto>("CreateNotificationDto");

    generator.add_schema::<AuditLogDto>("AuditLogDto");

    // Enums
    generator.add_schema::<TapOccupation>("TapOccupation");
    generator.add_schema::<TapPermission>("TapPermission");
    generator.add_schema::<TapRole>("TapRole");

    let typescript = generator.generate().replace("r#type", "type");

    // Post-process to convert snake_case keys to camelCase in Zod objects
    // This is a simple regex-based converter for keys in z.object({...})
    let mut processed = String::new();
    let mut in_object = 0;
    for line in typescript.lines() {
        if line.contains("z.object({") {
            in_object += 1;
            processed.push_str(line);
            processed.push('\n');
            continue;
        }

        if in_object > 0 && (line.trim() == "})" || line.trim() == "})," || line.trim() == "});") {
            in_object -= 1;
            processed.push_str(line);
            processed.push('\n');
            continue;
        }

        if in_object > 0 {
            // Match "  key_name: " and convert key_name to camelCase
            if let Some(colon_idx) = line.find(':') {
                let key_part = &line[..colon_idx];
                let val_part = &line[colon_idx..];

                let mut new_key = String::new();
                let mut next_upper = false;
                for c in key_part.chars() {
                    if c == '_' {
                        next_upper = true;
                    } else if next_upper {
                        new_key.push(c.to_ascii_uppercase());
                        next_upper = false;
                    } else {
                        new_key.push(c);
                    }
                }
                processed.push_str(&new_key);
                processed.push_str(val_part);
                processed.push('\n');
            } else {
                processed.push_str(line);
                processed.push('\n');
            }
        } else {
            processed.push_str(line);
            processed.push('\n');
        }
    }

    // Also fix known PascalCase literals in discriminated unions/unions
    let mut typescript = processed
        .replace("'OwnerOnly'", "'owner_only'")
        .replace("'Public'", "'public'")
        .replace("'Official'", "'official'")
        .replace("'Verified'", "'verified'")
        .replace("'Base'", "'base'")
        .replace("'Music'", "'music'")
        .replace("'TTS'", "'tts'")
        .replace("api_key:", "apiKey:")
        .replace("use_rate_history:", "useRateHistory:")
        .replace("cache_hit_rate_history:", "cacheHitRateHistory:")
        .replace("has_access:", "hasAccess:");

    typescript.push_str("\n// Manual additions for generic types\n");
    typescript.push_str(
        "export interface PaginatedResponseDto<T> {\n  data: T[];\n  meta: PaginationMetaDto;\n}\n",
    );

    // The output path is ../packages/zako3-data/src/generated/hq.ts relative to the workspace root.
    // We can find the workspace root by looking at CARGO_MANIFEST_DIR and going up one level.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let mut path = PathBuf::from(manifest_dir);
    path.push("..");
    path.push("packages");
    path.push("zako3-data");
    path.push("src");
    path.push("generated");

    fs::create_dir_all(&path).expect("Failed to create directory");

    path.push("hq.ts");
    fs::write(&path, typescript).expect("Failed to write typescript file");

    println!("Successfully generated Zod schemas at {:?}", path);
}
