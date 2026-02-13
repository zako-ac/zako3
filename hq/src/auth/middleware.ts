import type {
  FastifyInstance,
  FastifyRequest,
  FastifyReply,
  FastifyPluginOptions,
} from 'fastify';
import { UserId } from 'zako3-settings';
import type { IJWTVerifier } from './jwt.js';
import { type AuthContext, AuthErrors } from './context.js';

declare module 'fastify' {
  interface FastifyRequest {
    auth?: AuthContext;
  }
}

export interface AuthMiddlewareConfig {
  jwtVerifier: IJWTVerifier;
  excludePaths?: string[];
}

function extractBearerToken(request: FastifyRequest): string | undefined {
  const authHeader = request.headers.authorization;
  if (!authHeader || !authHeader.startsWith('Bearer ')) {
    return undefined;
  }
  return authHeader.slice(7);
}

/**
 * Registers the auth middleware hook directly on the Fastify instance.
 * This hook will run for all routes registered after this call,
 * including routes in child contexts (plugins with prefixes).
 *
 * IMPORTANT: Must be called BEFORE registering routes, and should be
 * called directly on the app instance, not inside a plugin.
 */
export function registerAuthMiddleware(
  fastify: FastifyInstance,
  config: AuthMiddlewareConfig
): void {
  const { jwtVerifier, excludePaths = [] } = config;

  fastify.addHook(
    'onRequest',
    async (request: FastifyRequest, reply: FastifyReply) => {
      const isExcluded = excludePaths.some(
        (path) =>
          request.url === path || request.url.startsWith(`${path}/`)
      );

      if (isExcluded) {
        return;
      }

      const token = extractBearerToken(request);
      if (!token) {
        return reply.status(401).send({
          error: 'Unauthorized',
          message: AuthErrors.MISSING_TOKEN,
        });
      }

      const result = await jwtVerifier.verify(token);
      if (!result.ok) {
        return reply.status(401).send({
          error: 'Unauthorized',
          message: result.error,
        });
      }

      request.auth = {
        userId: UserId(result.value.sub),
        raw: result.value,
      };
    }
  );
}

/**
 * @deprecated Use registerAuthMiddleware instead.
 * This plugin-based approach has encapsulation issues with child contexts.
 */
export function authMiddleware(config: AuthMiddlewareConfig) {
  const { jwtVerifier, excludePaths = [] } = config;

  return async function (
    fastify: FastifyInstance,
    _opts: FastifyPluginOptions
  ) {
    registerAuthMiddleware(fastify, { jwtVerifier, excludePaths });
  };
}

export function requireAuth(
  request: FastifyRequest,
  reply: FastifyReply
): AuthContext | undefined {
  if (!request.auth) {
    reply.status(401).send({
      error: 'Unauthorized',
      message: AuthErrors.MISSING_TOKEN,
    });
    return undefined;
  }
  return request.auth;
}
