import type { FastifyInstance, FastifyPluginOptions } from "fastify";
import type { ZodTypeProvider } from "fastify-type-provider-zod";
import type { Logger } from "pino";
import {
    TapService,
    type CreateTapRequest,
    type UpdateTapRequest,
} from "../services/tap.service.js";
import { requireAuth } from "./context.js";
import { isOk } from "../lib/result.js";
import {
    NotFoundError,
    ForbiddenError,
    ValidationError,
    AppError,
} from "../lib/errors.js";
import {
    errorResponseSchema,
    createDataResponseSchema,
    createPaginatedResponseSchema,
    noContentResponseSchema,
} from "../schemas/common.js";
import {
    tapSchema,
    tapMemberSchema,
    createTapRequestSchema,
    updateTapRequestSchema,
    listTapsQuerySchema,
    tapIdParamSchema,
} from "../schemas/tap.js";

export interface TapRouteDependencies {
    tapService: TapService;
    logger: Logger;
}

/**
 * Tap API routes
 */
export function tapRoutes(deps: TapRouteDependencies) {
    return async function (
        fastify: FastifyInstance,
        _opts: FastifyPluginOptions
    ) {
        const server = fastify.withTypeProvider<ZodTypeProvider>();
        const log = deps.logger.child({ routes: "taps" });

        /**
         * GET /taps
         * List taps with filtering and pagination
         */
        server.get(
            "/taps",
            {
                schema: {
                    tags: ["taps"],
                    summary: "List taps",
                    description:
                        "Get a paginated list of taps with optional filtering",
                    querystring: listTapsQuerySchema,
                    response: {
                        200: createPaginatedResponseSchema(tapSchema),
                        500: errorResponseSchema,
                    },
                },
            },
            async (request, reply) => {
                try {
                    const { page, limit, search, isVerified, ownerId } =
                        request.query;

                    const result = await deps.tapService.listTaps({
                        page: page ? Number(page) : undefined,
                        limit: limit ? Number(limit) : undefined,
                        search,
                        isVerified,
                        ownerId,
                    });

                    if (!isOk(result)) {
                        return reply.status(500).send({
                            error: {
                                code: "INTERNAL_ERROR",
                                message: "Failed to list taps",
                            },
                        });
                    }

                    return reply.send(result.value);
                } catch (error) {
                    log.error({ error }, "Failed to list taps");
                    return reply.status(500).send({
                        error: {
                            code: "INTERNAL_ERROR",
                            message: "Failed to list taps",
                        },
                    });
                }
            }
        );

        /**
         * POST /taps
         * Create a new tap
         */
        server.post(
            "/taps",
            {
                schema: {
                    tags: ["taps"],
                    summary: "Create a tap",
                    description: "Create a new tap (community)",
                    body: createTapRequestSchema,
                    response: {
                        201: createDataResponseSchema(tapSchema),
                        400: errorResponseSchema,
                        401: errorResponseSchema,
                        500: errorResponseSchema,
                    },
                    security: [{ bearerAuth: [] }],
                },
            },
            async (request, reply) => {
                try {
                    const auth = requireAuth(request);
                    const result = await deps.tapService.createTap(
                        request.body,
                        auth.userId
                    );

                    if (!isOk(result)) {
                        if (result.error instanceof ValidationError) {
                            return reply.status(400).send({
                                error: {
                                    code: "VALIDATION_ERROR",
                                    message: result.error.message,
                                },
                            });
                        }
                        return reply.status(500).send({
                            error: {
                                code: "INTERNAL_ERROR",
                                message: "Failed to create tap",
                            },
                        });
                    }

                    return reply.status(201).send({ data: result.value });
                } catch (error) {
                    log.error({ error }, "Failed to create tap");
                    return reply.status(500).send({
                        error: {
                            code: "INTERNAL_ERROR",
                            message: "Failed to create tap",
                        },
                    });
                }
            }
        );

        /**
         * GET /taps/:tapId
         * Get details of a specific tap
         */
        server.get(
            "/taps/:tapId",
            {
                schema: {
                    tags: ["taps"],
                    summary: "Get tap details",
                    description:
                        "Get detailed information about a specific tap",
                    params: tapIdParamSchema,
                    response: {
                        200: createDataResponseSchema(tapSchema),
                        401: errorResponseSchema,
                        403: errorResponseSchema,
                        404: errorResponseSchema,
                        500: errorResponseSchema,
                    },
                    security: [{ bearerAuth: [] }],
                },
            },
            async (request, reply) => {
                try {
                    const { tapId } = request.params;
                    const auth = requireAuth(request);

                    const result = await deps.tapService.getTap(
                        tapId,
                        auth.userId
                    );

                    if (!isOk(result)) {
                        if (result.error instanceof NotFoundError) {
                            return reply.status(404).send({
                                error: {
                                    code: "TAP_NOT_FOUND",
                                    message: result.error.message,
                                },
                            });
                        }
                        if (result.error instanceof ForbiddenError) {
                            return reply.status(403).send({
                                error: {
                                    code: "FORBIDDEN",
                                    message: result.error.message,
                                },
                            });
                        }
                        return reply.status(500).send({
                            error: {
                                code: "INTERNAL_ERROR",
                                message: "Failed to get tap",
                            },
                        });
                    }

                    return reply.send({ data: result.value });
                } catch (error) {
                    log.error({ error }, "Failed to get tap");
                    return reply.status(500).send({
                        error: {
                            code: "INTERNAL_ERROR",
                            message: "Failed to get tap",
                        },
                    });
                }
            }
        );

        /**
         * PATCH /taps/:tapId
         * Update a tap
         */
        server.patch(
            "/taps/:tapId",
            {
                schema: {
                    tags: ["taps"],
                    summary: "Update tap",
                    description: "Update tap properties (owner or admin only)",
                    params: tapIdParamSchema,
                    body: updateTapRequestSchema,
                    response: {
                        200: createDataResponseSchema(tapSchema),
                        400: errorResponseSchema,
                        401: errorResponseSchema,
                        403: errorResponseSchema,
                        404: errorResponseSchema,
                        500: errorResponseSchema,
                    },
                    security: [{ bearerAuth: [] }],
                },
            },
            async (request, reply) => {
                try {
                    const { tapId } = request.params;
                    const auth = requireAuth(request);

                    const result = await deps.tapService.updateTap(
                        tapId,
                        request.body,
                        auth.userId
                    );

                    if (!isOk(result)) {
                        if (result.error instanceof NotFoundError) {
                            return reply.status(404).send({
                                error: {
                                    code: "TAP_NOT_FOUND",
                                    message: result.error.message,
                                },
                            });
                        }
                        if (result.error instanceof ForbiddenError) {
                            return reply.status(403).send({
                                error: {
                                    code: "FORBIDDEN",
                                    message: result.error.message,
                                },
                            });
                        }
                        if (result.error instanceof ValidationError) {
                            return reply.status(400).send({
                                error: {
                                    code: "VALIDATION_ERROR",
                                    message: result.error.message,
                                },
                            });
                        }
                        return reply.status(500).send({
                            error: {
                                code: "INTERNAL_ERROR",
                                message: "Failed to update tap",
                            },
                        });
                    }

                    return reply.send({ data: result.value });
                } catch (error) {
                    log.error({ error }, "Failed to update tap");
                    return reply.status(500).send({
                        error: {
                            code: "INTERNAL_ERROR",
                            message: "Failed to update tap",
                        },
                    });
                }
            }
        );

        /**
         * DELETE /taps/:tapId
         * Delete a tap
         */
        server.delete(
            "/taps/:tapId",
            {
                schema: {
                    tags: ["taps"],
                    summary: "Delete tap",
                    description: "Delete a tap (owner only)",
                    params: tapIdParamSchema,
                    response: {
                        204: noContentResponseSchema,
                        401: errorResponseSchema,
                        403: errorResponseSchema,
                        404: errorResponseSchema,
                        500: errorResponseSchema,
                    },
                    security: [{ bearerAuth: [] }],
                },
            },
            async (request, reply) => {
                try {
                    const { tapId } = request.params;
                    const auth = requireAuth(request);

                    const result = await deps.tapService.deleteTap(
                        tapId,
                        auth.userId
                    );

                    if (!isOk(result)) {
                        if (result.error instanceof NotFoundError) {
                            return reply.status(404).send({
                                error: {
                                    code: "TAP_NOT_FOUND",
                                    message: result.error.message,
                                },
                            });
                        }
                        if (result.error instanceof ForbiddenError) {
                            return reply.status(403).send({
                                error: {
                                    code: "FORBIDDEN",
                                    message: result.error.message,
                                },
                            });
                        }
                        return reply.status(500).send({
                            error: {
                                code: "INTERNAL_ERROR",
                                message: "Failed to delete tap",
                            },
                        });
                    }

                    return reply.status(204).send();
                } catch (error) {
                    log.error({ error }, "Failed to delete tap");
                    return reply.status(500).send({
                        error: {
                            code: "INTERNAL_ERROR",
                            message: "Failed to delete tap",
                        },
                    });
                }
            }
        );

        /**
         * GET /taps/:tapId/members
         * Get tap members
         */
        server.get(
            "/taps/:tapId/members",
            {
                schema: {
                    tags: ["taps"],
                    summary: "Get tap members",
                    description: "Get list of tap members",
                    params: tapIdParamSchema,
                    response: {
                        200: createDataResponseSchema(tapMemberSchema.array()),
                        401: errorResponseSchema,
                        403: errorResponseSchema,
                        404: errorResponseSchema,
                        500: errorResponseSchema,
                    },
                    security: [{ bearerAuth: [] }],
                },
            },
            async (request, reply) => {
                try {
                    const { tapId } = request.params;
                    const auth = requireAuth(request);

                    const result = await deps.tapService.getTapMembers(
                        tapId,
                        auth.userId
                    );

                    if (!isOk(result)) {
                        if (result.error instanceof NotFoundError) {
                            return reply.status(404).send({
                                error: {
                                    code: "TAP_NOT_FOUND",
                                    message: result.error.message,
                                },
                            });
                        }
                        if (result.error instanceof ForbiddenError) {
                            return reply.status(403).send({
                                error: {
                                    code: "FORBIDDEN",
                                    message: result.error.message,
                                },
                            });
                        }
                        return reply.status(500).send({
                            error: {
                                code: "INTERNAL_ERROR",
                                message: "Failed to get tap members",
                            },
                        });
                    }

                    return reply.send({ data: result.value });
                } catch (error) {
                    log.error({ error }, "Failed to get tap members");
                    return reply.status(500).send({
                        error: {
                            code: "INTERNAL_ERROR",
                            message: "Failed to get tap members",
                        },
                    });
                }
            }
        );
    };
}
