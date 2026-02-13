#!/usr/bin/env tsx
/**
 * Export OpenAPI specification to JSON file
 *
 * Usage:
 *   pnpm run openapi:export
 */

import { writeFileSync, mkdirSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";
import Fastify from "fastify";
import fastifySwagger from "@fastify/swagger";
import {
    serializerCompiler,
    validatorCompiler,
    jsonSchemaTransform,
    type ZodTypeProvider,
} from "fastify-type-provider-zod";
import { registerRoutes } from "../src/routes/index.js";
import {
    mockLogger,
    mockDatabase,
    mockRedis,
    mockSettingsService,
    mockSettingsOperations,
    mockUserService,
    mockTapService,
} from "./mocks.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

async function exportOpenAPI() {
    const fastify = Fastify({
        logger: false,
    }).withTypeProvider<ZodTypeProvider>();

    // Set up Zod validation and serialization
    fastify.setValidatorCompiler(validatorCompiler);
    fastify.setSerializerCompiler(serializerCompiler);

    // Register swagger with OpenAPI configuration
    await fastify.register(fastifySwagger, {
        openapi: {
            openapi: "3.0.3",
            info: {
                title: "Zako3 HQ API",
                description:
                    "RESTful API for Zako3 platform - manage users, communities (taps), and bot settings",
                version: "1.0.0",
                contact: {
                    name: "Zako3 Team",
                },
                license: {
                    name: "ISC",
                },
            },
            servers: [
                {
                    url: "http://localhost:3000",
                    description: "Local development server",
                },
                {
                    url: "https://api.zako3.com",
                    description: "Production server",
                },
            ],
            tags: [
                { name: "health", description: "Health check endpoints" },
                { name: "users", description: "User management" },
                { name: "taps", description: "Tap (community) management" },
                { name: "settings", description: "Bot settings management" },
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

    // Register all routes with mock dependencies
    await fastify.register(
        registerRoutes({
            database: mockDatabase,
            redis: mockRedis,
            settings: mockSettingsService,
            operations: mockSettingsOperations,
            userService: mockUserService,
            tapService: mockTapService,
            logger: mockLogger,
        })
    );

    await fastify.ready();

    // Get the OpenAPI spec
    const spec = fastify.swagger();

    // Create output directory if it doesn't exist
    const outputDir = join(__dirname, "..", "docs");
    mkdirSync(outputDir, { recursive: true });

    // Write JSON file
    const jsonPath = join(outputDir, "openapi.json");
    writeFileSync(jsonPath, JSON.stringify(spec, null, 2));
    console.log(`✅ OpenAPI specification exported to: ${jsonPath}`);

    // Count and display statistics
    const paths = spec.paths ? Object.keys(spec.paths) : [];
    const endpoints = paths.reduce((count, path) => {
        return count + Object.keys(spec.paths?.[path] || {}).length;
    }, 0);

    console.log(
        `📊 Exported ${endpoints} endpoints across ${paths.length} paths`
    );
    console.log("\n📋 Endpoints by tag:");

    const endpointsByTag: Record<string, number> = {};
    if (spec.paths) {
        for (const path in spec.paths) {
            const pathItem = spec.paths[path];
            if (pathItem && typeof pathItem === "object") {
                for (const method in pathItem) {
                    const endpoint = pathItem[method];
                    if (endpoint && typeof endpoint === "object") {
                        const tags = (endpoint as any).tags || ["untagged"];
                        for (const tag of tags) {
                            endpointsByTag[tag] =
                                (endpointsByTag[tag] || 0) + 1;
                        }
                    }
                }
            }
        }
    }

    for (const [tag, count] of Object.entries(endpointsByTag).sort()) {
        console.log(`   ${tag}: ${count} endpoints`);
    }

    await fastify.close();
}

exportOpenAPI().catch((error) => {
    console.error("Failed to export OpenAPI specification:", error);
    process.exit(1);
});
