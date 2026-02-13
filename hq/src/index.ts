import { initTracing, shutdownTracing } from "./infra/tracing.js";
import { loadEnv } from "./config/env.js";

const env = loadEnv();

initTracing({
    serviceName: env.OTEL_SERVICE_NAME,
    serviceVersion: env.OTEL_SERVICE_VERSION,
    environment: env.NODE_ENV,
    otlpEndpoint: env.OTEL_EXPORTER_OTLP_ENDPOINT,
    debug: env.OTEL_DEBUG,
});

import Fastify from "fastify";
import fastifySwagger from "@fastify/swagger";
import fastifySwaggerUi from "@fastify/swagger-ui";
import {
    serializerCompiler,
    validatorCompiler,
    jsonSchemaTransform,
    type ZodTypeProvider,
} from "fastify-type-provider-zod";
import { Client, GatewayIntentBits } from "discord.js";
import {
    UserId,
    GuildId,
    ADMIN_KEY_ADMINS,
    adminContext,
} from "zako3-settings";
import { createDatabase, createRedis, createLogger } from "./infra/index.js";
import {
    createSettingsService,
    createSettingsOperations,
} from "./service/index.js";
import { registerRoutes } from "./routes/index.js";
import { UserRepository } from "./repositories/user.repository.js";
import { TapRepository } from "./repositories/tap.repository.js";
import { UserService } from "./services/user.service.js";
import { TapService } from "./services/tap.service.js";
import {
    createJWTVerifier,
    createStaticJWTVerifier,
    authMiddleware,
    type IJWTVerifier,
    type IGuildPermissionChecker,
    type IBotAdminChecker,
} from "./auth/index.js";

async function main() {
    const logger = createLogger({
        serviceName: env.OTEL_SERVICE_NAME,
        serviceVersion: env.OTEL_SERVICE_VERSION,
        environment: env.NODE_ENV,
        level: env.LOG_LEVEL,
        otlpEndpoint: env.OTEL_EXPORTER_OTLP_ENDPOINT,
        prettyPrint: env.NODE_ENV === "development",
    });

    logger.info({ env: env.NODE_ENV }, "Starting zako3-hq");

    const database = createDatabase({ url: env.DATABASE_URL }, logger);
    const redis = createRedis({ url: env.REDIS_URL }, logger);

    const settings = createSettingsService({
        database,
        redis,
        logger,
        cacheTtlMs: env.SETTINGS_CACHE_TTL_MS,
    });

    const initResult = await settings.initialize();
    if (!initResult.ok) {
        logger.fatal(
            { error: initResult.error },
            "Failed to initialize settings service"
        );
        process.exit(1);
    }

    const discord = new Client({
        intents: [GatewayIntentBits.Guilds, GatewayIntentBits.GuildMembers],
    });

    const guildChecker: IGuildPermissionChecker = {
        async isGuildAdmin(userId: UserId, guildId: GuildId): Promise<boolean> {
            if (!discord.isReady()) {
                return false;
            }
            try {
                const guild = await discord.guilds.fetch(guildId as string);
                const member = await guild.members.fetch(userId as string);
                return member.permissions.has("Administrator");
            } catch {
                return false;
            }
        },
    };

    const adminChecker: IBotAdminChecker = {
        async isBotAdmin(userId: UserId): Promise<boolean> {
            const result = await settings.get(ADMIN_KEY_ADMINS, adminContext());
            if (!result.ok) {
                return false;
            }
            const admins = result.value.value as string[];
            return admins.includes(userId as string);
        },
    };

    const operations = createSettingsOperations({
        settingsService: settings,
        guildChecker,
        adminChecker,
    });

    // Create repositories and services for API
    const userRepo = new UserRepository(database, redis.client, logger);
    const tapRepo = new TapRepository(database, redis.client, logger);
    const userService = new UserService(userRepo, logger);
    const tapService = new TapService(tapRepo, userRepo, logger);

    let jwtVerifier: IJWTVerifier;
    if (env.JWT_JWKS_URL) {
        jwtVerifier = createJWTVerifier({
            jwksUrl: env.JWT_JWKS_URL,
            issuer: env.JWT_ISSUER,
            audience: env.JWT_AUDIENCE,
        });
    } else if (env.JWT_PUBLIC_KEY) {
        jwtVerifier = createStaticJWTVerifier({
            publicKey: env.JWT_PUBLIC_KEY,
            algorithm: env.JWT_ALGORITHM,
            issuer: env.JWT_ISSUER,
            audience: env.JWT_AUDIENCE,
        });
    } else {
        logger.warn(
            "No JWT configuration provided, API authentication will fail"
        );
        jwtVerifier = {
            async verify() {
                return { ok: false as const, error: "JWT not configured" };
            },
        };
    }

    const fastify = Fastify({
        logger: logger.child({ module: "fastify" }),
    }).withTypeProvider<ZodTypeProvider>();

    // Set up Zod validation and serialization
    fastify.setValidatorCompiler(validatorCompiler);
    fastify.setSerializerCompiler(serializerCompiler);

    if (env.SWAGGER_ENABLED || env.NODE_ENV === "development") {
        await fastify.register(fastifySwagger, {
            openapi: {
                openapi: "3.0.3",
                info: {
                    title: "Zako3 HQ API",
                    description:
                        "RESTful API for Zako3 platform - manage users, communities (taps), and bot settings",
                    version: env.OTEL_SERVICE_VERSION,
                    contact: {
                        name: "Zako3 Team",
                    },
                    license: {
                        name: "ISC",
                    },
                },
                servers: [
                    {
                        url: `http://localhost:${env.PORT}`,
                        description: "Local development server",
                    },
                    {
                        url: `https://api.zako3.com`,
                        description: "Production server",
                    },
                ],
                tags: [
                    { name: "health", description: "Health check endpoints" },
                    { name: "users", description: "User management" },
                    { name: "taps", description: "Tap (community) management" },
                    {
                        name: "settings",
                        description: "Bot settings management",
                    },
                ],
                components: {
                    securitySchemes: {
                        bearerAuth: {
                            type: "http",
                            scheme: "bearer",
                            bearerFormat: "JWT",
                            description:
                                "JWT token obtained from authentication provider",
                        },
                    },
                },
                externalDocs: {
                    description: "Zako3 Documentation",
                    url: "https://docs.zako3.com",
                },
            },
            transform: jsonSchemaTransform,
        });

        await fastify.register(fastifySwaggerUi, {
            routePrefix: "/docs",
            uiConfig: {
                docExpansion: "list",
                deepLinking: true,
            },
        });
    }

    await fastify.register(
        authMiddleware({
            jwtVerifier,
            excludePaths: ["/healthz", "/docs"],
        })
    );

    await fastify.register(
        registerRoutes({
            database,
            redis,
            settings,
            operations,
            userService,
            tapService,
            logger,
        })
    );

    const shutdown = async (signal: string) => {
        logger.info({ signal }, "Received shutdown signal");

        await fastify.close();
        discord.destroy();
        await settings.shutdown();
        await redis.close();
        await database.close();
        await shutdownTracing();

        logger.info("Shutdown complete");
        process.exit(0);
    };

    process.on("SIGTERM", () => shutdown("SIGTERM"));
    process.on("SIGINT", () => shutdown("SIGINT"));

    try {
        await fastify.listen({ port: env.PORT, host: "0.0.0.0" });
        logger.info({ port: env.PORT }, "Fastify server started");

        if (env.DISCORD_TOKEN) {
            await discord.login(env.DISCORD_TOKEN);
            logger.info("Discord bot logged in");
        } else {
            logger.warn("No DISCORD_TOKEN, bot not started");
        }
    } catch (err) {
        logger.fatal({ err }, "Failed to start");
        process.exit(1);
    }
}

main();
