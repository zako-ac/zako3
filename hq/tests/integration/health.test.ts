import { describe, it, expect, beforeAll, afterAll, beforeEach } from 'vitest';
import { createTestContext, destroyTestContext, type TestContext } from './helpers.js';

describe('Health Endpoints', () => {
  let ctx: TestContext;

  beforeAll(async () => {
    ctx = await createTestContext();
  });

  afterAll(async () => {
    await destroyTestContext(ctx);
  });

  beforeEach(async () => {
    await ctx.cleanup();
  });

  describe('GET /healthz', () => {
    it('returns ok status', async () => {
      const response = await ctx.app.inject({
        method: 'GET',
        url: '/healthz',
      });

      expect(response.statusCode).toBe(200);
      expect(response.json()).toEqual({ status: 'ok' });
    });
  });

  describe('GET /healthz/live', () => {
    it('returns ok status for liveness probe', async () => {
      const response = await ctx.app.inject({
        method: 'GET',
        url: '/healthz/live',
      });

      expect(response.statusCode).toBe(200);
      expect(response.json()).toEqual({ status: 'ok' });
    });
  });

  describe('GET /healthz/ready', () => {
    it('returns healthy status when all services are up', async () => {
      const response = await ctx.app.inject({
        method: 'GET',
        url: '/healthz/ready',
      });

      expect(response.statusCode).toBe(200);
      const body = response.json();
      expect(body.status).toBe('healthy');
      expect(body.checks.database.status).toBe('up');
      expect(body.checks.redis.status).toBe('up');
      expect(body.checks.settings.status).toBe('up');
      expect(body.timestamp).toBeDefined();
      expect(body.uptime).toBeGreaterThanOrEqual(0);
    });
  });
});
