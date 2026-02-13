import type { FastifyRequest } from 'fastify';

/**
 * Request context with authenticated user info
 */
export interface RequestContext {
  userId: string;
  isAdmin: boolean;
}

/**
 * Extended Fastify request with request context
 */
export interface AuthenticatedRequest extends FastifyRequest {
  requestContext: RequestContext;
}

/**
 * Check if request is authenticated
 */
export function isAuthenticated(
  request: FastifyRequest,
): request is AuthenticatedRequest {
  return 'requestContext' in request && request.requestContext !== undefined;
}

/**
 * Get request context from request or throw
 */
export function requireAuth(request: FastifyRequest): RequestContext {
  if (!isAuthenticated(request)) {
    throw new Error('Request is not authenticated');
  }
  return request.requestContext;
}
