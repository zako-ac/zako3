import { z } from "zod";

/**
 * Component health status
 */
export const componentHealthSchema = z.object({
    status: z.enum(["up", "down"]).describe("Component health status"),
    latencyMs: z
        .number()
        .optional()
        .describe("Response latency in milliseconds"),
});

export type ComponentHealth = z.infer<typeof componentHealthSchema>;

/**
 * Health check status
 */
export const healthStatusSchema = z.object({
    status: z
        .enum(["healthy", "degraded", "unhealthy"])
        .describe("Overall system health status"),
    timestamp: z.string().describe("ISO 8601 timestamp"),
    uptime: z.number().describe("Service uptime in seconds"),
    checks: z.object({
        database: componentHealthSchema,
        redis: componentHealthSchema,
        settings: componentHealthSchema,
    }),
});

export type HealthStatus = z.infer<typeof healthStatusSchema>;

/**
 * Basic health response
 */
export const basicHealthSchema = z.object({
    status: z.literal("ok"),
});

export type BasicHealth = z.infer<typeof basicHealthSchema>;
