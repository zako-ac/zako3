import type { FastifyInstance, FastifyPluginOptions } from "fastify";
import type { ZodTypeProvider } from "fastify-type-provider-zod";
import type { Database } from "../infra/database.js";
import type { RedisClient } from "../infra/redis.js";
import type { SettingsService } from "../service/settings/index.js";
import {
    basicHealthSchema,
    healthStatusSchema,
    type ComponentHealth,
    type HealthStatus,
} from "../schemas/health.js";

export interface HealthDependencies {
    database: Database;
    redis: RedisClient;
    settings: SettingsService;
}

async function checkWithLatency(
    fn: () => Promise<boolean>
): Promise<ComponentHealth> {
    const start = Date.now();
    try {
        const healthy = await fn();
        return {
            status: healthy ? "up" : "down",
            latencyMs: Date.now() - start,
        };
    } catch {
        return {
            status: "down",
            latencyMs: Date.now() - start,
        };
    }
}

export function healthRoutes(deps: HealthDependencies) {
    const startTime = Date.now();

    return async function (
        fastify: FastifyInstance,
        _opts: FastifyPluginOptions
    ) {
        const server = fastify.withTypeProvider<ZodTypeProvider>();

        server.get(
            "/healthz",
            {
                schema: {
                    tags: ["health"],
                    summary: "Basic health check",
                    description: "Returns OK if the service is running",
                    response: {
                        200: basicHealthSchema,
                    },
                },
            },
            async (_request, reply) => {
                return reply.status(200).send({ status: "ok" });
            }
        );

        server.get(
            "/healthz/live",
            {
                schema: {
                    tags: ["health"],
                    summary: "Liveness probe",
                    description:
                        "Kubernetes liveness probe - returns OK if the service is alive",
                    response: {
                        200: basicHealthSchema,
                    },
                },
            },
            async (_request, reply) => {
                return reply.status(200).send({ status: "ok" });
            }
        );

        server.get(
            "/healthz/ready",
            {
                schema: {
                    tags: ["health"],
                    summary: "Readiness probe",
                    description:
                        "Kubernetes readiness probe - checks if all dependencies are healthy",
                    response: {
                        200: healthStatusSchema,
                        503: healthStatusSchema,
                    },
                },
            },
            async (_request, reply) => {
                const [database, redis, settings] = await Promise.all([
                    checkWithLatency(() => deps.database.isHealthy()),
                    checkWithLatency(() => deps.redis.isHealthy()),
                    checkWithLatency(() => deps.settings.isHealthy()),
                ]);

                const checks = { database, redis, settings };
                const statuses = Object.values(checks).map((c) => c.status);
                const allUp = statuses.every((s) => s === "up");
                const allDown = statuses.every((s) => s === "down");

                const status: HealthStatus = {
                    status: allUp
                        ? "healthy"
                        : allDown
                          ? "unhealthy"
                          : "degraded",
                    timestamp: new Date().toISOString(),
                    uptime: Math.floor((Date.now() - startTime) / 1000),
                    checks,
                };

                const httpStatus = status.status === "unhealthy" ? 503 : 200;
                return reply.status(httpStatus).send(status);
            }
        );
    };
}
