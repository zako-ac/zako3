import type { FastifyInstance, FastifyPluginOptions } from "fastify";
import type { ZodTypeProvider } from "fastify-type-provider-zod";
import type { SettingsOperations } from "../service/settings/index.js";
import { requireAuth } from "../auth/index.js";
import { errorResponseSchema } from "../schemas/common.js";
import {
    settingKeyIdParamSchema,
    getSettingQuerySchema,
    setSettingRequestSchema,
    setSettingResponseSchema,
    deleteSettingQuerySchema,
    listSettingsQuerySchema,
    settingValueSchema,
    deleteSettingResponseSchema,
    listSettingsResponseSchema,
} from "../schemas/settings.js";

export interface SettingsRouteDependencies {
    operations: SettingsOperations;
}

export function settingsRoutes(deps: SettingsRouteDependencies) {
    const { operations } = deps;

    return async function (
        fastify: FastifyInstance,
        _opts: FastifyPluginOptions
    ) {
        const server = fastify.withTypeProvider<ZodTypeProvider>();

        server.get(
            "/settings/:keyId",
            {
                schema: {
                    tags: ["settings"],
                    summary: "Get a setting value",
                    description:
                        "Retrieves the resolved value for a setting key, considering scope cascading.",
                    params: settingKeyIdParamSchema,
                    querystring: getSettingQuerySchema,
                    response: {
                        200: settingValueSchema,
                        400: errorResponseSchema,
                        401: errorResponseSchema,
                        403: errorResponseSchema,
                        404: errorResponseSchema,
                    },
                    security: [{ bearerAuth: [] }],
                },
            },
            async (request, reply) => {
                const auth = requireAuth(request, reply);
                if (!auth) return;

                const result = await operations.get(
                    {
                        keyId: request.params.keyId,
                        userId: request.query.userId,
                        guildId: request.query.guildId,
                    },
                    auth
                );

                if (!result.ok) {
                    const statusCode = result.error.includes("not found")
                        ? 404
                        : result.error.includes("denied")
                          ? 403
                          : 400;
                    return reply.status(statusCode).send({
                        error: {
                            code:
                                statusCode === 404
                                    ? "NOT_FOUND"
                                    : statusCode === 403
                                      ? "FORBIDDEN"
                                      : "BAD_REQUEST",
                            message: result.error,
                        },
                    });
                }

                return reply.send(result.value);
            }
        );

        server.put(
            "/settings/:keyId",
            {
                schema: {
                    tags: ["settings"],
                    summary: "Set a setting value",
                    description:
                        "Sets a value for a setting key at the specified scope.",
                    params: settingKeyIdParamSchema,
                    body: setSettingRequestSchema,
                    response: {
                        204: setSettingResponseSchema,
                        400: errorResponseSchema,
                        401: errorResponseSchema,
                        403: errorResponseSchema,
                        404: errorResponseSchema,
                    },
                    security: [{ bearerAuth: [] }],
                },
            },
            async (request, reply) => {
                const auth = requireAuth(request, reply);
                if (!auth) return;

                const result = await operations.set(
                    {
                        keyId: request.params.keyId,
                        value: request.body.value,
                        scopeType: request.body.scopeType as Parameters<
                            typeof operations.set
                        >[0]["scopeType"],
                        userId: request.body.userId,
                        guildId: request.body.guildId,
                    },
                    auth
                );

                if (!result.ok) {
                    const statusCode = result.error.includes("not found")
                        ? 404
                        : result.error.includes("denied")
                          ? 403
                          : 400;
                    return reply.status(statusCode).send({
                        error: {
                            code:
                                statusCode === 404
                                    ? "NOT_FOUND"
                                    : statusCode === 403
                                      ? "FORBIDDEN"
                                      : "BAD_REQUEST",
                            message: result.error,
                        },
                    });
                }

                return reply.status(204).send();
            }
        );

        server.delete(
            "/settings/:keyId",
            {
                schema: {
                    tags: ["settings"],
                    summary: "Delete a setting value",
                    description:
                        "Removes a setting value at the specified scope, reverting to the next level in the cascade.",
                    params: settingKeyIdParamSchema,
                    querystring: deleteSettingQuerySchema,
                    response: {
                        200: deleteSettingResponseSchema,
                        400: errorResponseSchema,
                        401: errorResponseSchema,
                        403: errorResponseSchema,
                        404: errorResponseSchema,
                    },
                    security: [{ bearerAuth: [] }],
                },
            },
            async (request, reply) => {
                const auth = requireAuth(request, reply);
                if (!auth) return;

                const result = await operations.delete(
                    {
                        keyId: request.params.keyId,
                        scopeType: request.query.scopeType as Parameters<
                            typeof operations.delete
                        >[0]["scopeType"],
                        userId: request.query.userId,
                        guildId: request.query.guildId,
                    },
                    auth
                );

                if (!result.ok) {
                    const statusCode = result.error.includes("not found")
                        ? 404
                        : result.error.includes("denied")
                          ? 403
                          : 400;
                    return reply.status(statusCode).send({
                        error: {
                            code:
                                statusCode === 404
                                    ? "NOT_FOUND"
                                    : statusCode === 403
                                      ? "FORBIDDEN"
                                      : "BAD_REQUEST",
                            message: result.error,
                        },
                    });
                }

                return reply.send({ deleted: result.value });
            }
        );

        server.get(
            "/settings",
            {
                schema: {
                    tags: ["settings"],
                    summary: "List settings",
                    description:
                        "Lists all settings for the specified kind and optional scope filter. Only accessible by admins or the resource owner.",
                    querystring: listSettingsQuerySchema,
                    response: {
                        200: listSettingsResponseSchema,
                        400: errorResponseSchema,
                        401: errorResponseSchema,
                        403: errorResponseSchema,
                    },
                    security: [{ bearerAuth: [] }],
                },
            },
            async (request, reply) => {
                const auth = requireAuth(request, reply);
                if (!auth) return;

                const result = await operations.list(
                    {
                        settingsKind: request.query.settingsKind as Parameters<
                            typeof operations.list
                        >[0]["settingsKind"],
                        scopeType: request.query.scopeType as
                            | Parameters<typeof operations.list>[0]["scopeType"]
                            | undefined,
                        userId: request.query.userId,
                        guildId: request.query.guildId,
                    },
                    auth
                );

                if (!result.ok) {
                    const statusCode = result.error.includes("denied")
                        ? 403
                        : 400;
                    return reply.status(statusCode).send({
                        error: {
                            code:
                                statusCode === 403
                                    ? "FORBIDDEN"
                                    : "BAD_REQUEST",
                            message: result.error,
                        },
                    });
                }

                return reply.send({ entries: result.value });
            }
        );
    };
}
