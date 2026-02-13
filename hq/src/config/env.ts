import 'dotenv/config'
import { z } from "zod";

const envSchema = z.object({
    DATABASE_URL: z.string().min(1),
    REDIS_URL: z.string().min(1),

    NODE_ENV: z
        .enum(["development", "production", "test"])
        .default("development"),
    PORT: z.coerce.number().int().min(1).max(65535).default(3000),
    LOG_LEVEL: z
        .enum(["trace", "debug", "info", "warn", "error", "fatal"])
        .default("info"),

    OTEL_EXPORTER_OTLP_ENDPOINT: z.string().url().optional(),
    OTEL_SERVICE_NAME: z.string().default("zako3-hq"),
    OTEL_SERVICE_VERSION: z.string().default("1.0.0"),
    OTEL_DEBUG: z
        .string()
        .optional()
        .default("false")
        .transform((v) => v === "true" || v === "1"),

    DISCORD_TOKEN: z.string().optional(),

    SETTINGS_CACHE_TTL_MS: z.coerce.number().int().min(1000).default(60000),

    JWT_JWKS_URL: z.string().url().optional(),
    JWT_PUBLIC_KEY: z.string().optional(),
    JWT_ALGORITHM: z.enum(["RS256", "ES256"]).default("RS256"),
    JWT_ISSUER: z.string().optional(),
    JWT_AUDIENCE: z.string().optional(),

    SWAGGER_ENABLED: z
        .string()
        .optional()
        .default("false")
        .transform((v) => v === "true" || v === "1"),
});

export type Env = z.infer<typeof envSchema>;

let cachedEnv: Env | null = null;

export function loadEnv(): Env {
    if (cachedEnv) return cachedEnv;

    const result = envSchema.safeParse(process.env);

    if (!result.success) {
        const formatted = result.error.issues
            .map((issue) => `  - ${issue.path.join(".")}: ${issue.message}`)
            .join("\n");

        throw new Error(`Environment validation failed:\n${formatted}`);
    }

    cachedEnv = result.data;
    return cachedEnv;
}

export function getEnv<K extends keyof Env>(key: K): Env[K] {
    return loadEnv()[key];
}

export function isProduction(): boolean {
    return loadEnv().NODE_ENV === "production";
}

export function isDevelopment(): boolean {
    return loadEnv().NODE_ENV === "development";
}
