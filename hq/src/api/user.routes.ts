import type { FastifyInstance, FastifyPluginOptions } from "fastify";
import type { ZodTypeProvider } from "fastify-type-provider-zod";
import type { Logger } from "pino";
import { UserService } from "../services/user.service.js";
import { requireAuth } from "./context.js";
import { isOk } from "../lib/result.js";
import { NotFoundError, UnauthorizedError, AppError } from "../lib/errors.js";
import {
    errorResponseSchema,
    createDataResponseSchema,
} from "../schemas/common.js";
import {
    userSchema,
    userProfileSchema,
    userIdParamSchema,
} from "../schemas/user.js";

export interface UserRouteDependencies {
    userService: UserService;
    logger: Logger;
}

/**
 * User API routes
 */
export function userRoutes(deps: UserRouteDependencies) {
    return async function (
        fastify: FastifyInstance,
        _opts: FastifyPluginOptions
    ) {
        const server = fastify.withTypeProvider<ZodTypeProvider>();
        const log = deps.logger.child({ routes: "users" });

        /**
         * GET /users/me
         * Get current authenticated user's profile
         */
        server.get(
            "/users/me",
            {
                schema: {
                    tags: ["users"],
                    summary: "Get current user profile",
                    description:
                        "Get the authenticated user's full profile including private information",
                    response: {
                        200: createDataResponseSchema(userProfileSchema),
                        401: errorResponseSchema,
                        404: errorResponseSchema,
                        500: errorResponseSchema,
                    },
                    security: [{ bearerAuth: [] }],
                },
            },
            async (request, reply) => {
                try {
                    const auth = requireAuth(request);
                    const result = await deps.userService.getUser(auth.userId);

                    if (!isOk(result)) {
                        if (result.error instanceof NotFoundError) {
                            return reply.status(404).send({
                                error: {
                                    code: "USER_NOT_FOUND",
                                    message: result.error.message,
                                },
                            });
                        }
                        return reply.status(500).send({
                            error: {
                                code: "INTERNAL_ERROR",
                                message: "Failed to get user",
                            },
                        });
                    }

                    return reply.send({ data: result.value });
                } catch (error) {
                    log.error({ error }, "Failed to get current user");

                    if (error instanceof UnauthorizedError) {
                        return reply.status(401).send({
                            error: {
                                code: "UNAUTHORIZED",
                                message: error.message,
                            },
                        });
                    }

                    return reply.status(500).send({
                        error: {
                            code: "INTERNAL_ERROR",
                            message: "Failed to get user",
                        },
                    });
                }
            }
        );

        /**
         * GET /users/:userId
         * Get public profile of a user
         */
        server.get(
            "/users/:userId",
            {
                schema: {
                    tags: ["users"],
                    summary: "Get user public profile",
                    description: "Get a public user profile by user ID",
                    params: userIdParamSchema,
                    response: {
                        200: createDataResponseSchema(userSchema),
                        404: errorResponseSchema,
                        500: errorResponseSchema,
                    },
                },
            },
            async (request, reply) => {
                try {
                    const { userId } = request.params;
                    const result =
                        await deps.userService.getPublicProfile(userId);

                    if (!isOk(result)) {
                        if (result.error instanceof NotFoundError) {
                            return reply.status(404).send({
                                error: {
                                    code: "USER_NOT_FOUND",
                                    message: result.error.message,
                                },
                            });
                        }
                        return reply.status(500).send({
                            error: {
                                code: "INTERNAL_ERROR",
                                message: "Failed to get user",
                            },
                        });
                    }

                    return reply.send({ data: result.value });
                } catch (error) {
                    log.error({ error }, "Failed to get user");

                    return reply.status(500).send({
                        error: {
                            code: "INTERNAL_ERROR",
                            message: "Failed to get user",
                        },
                    });
                }
            }
        );
    };
}
