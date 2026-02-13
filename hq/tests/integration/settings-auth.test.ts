import { describe, it, expect, beforeAll, afterAll, beforeEach } from 'vitest';
import { createTestContext, destroyTestContext, type TestContext, type MockPermissions } from './helpers.js';

describe('Settings Authorization', () => {
  let ctx: TestContext;
  const userA = '111111111';
  const userB = '222222222';
  const botAdmin = '999999999';
  const guildId = '555555555';

  const permissions: MockPermissions = {
    botAdmins: new Set([botAdmin]),
    guildAdmins: new Map([[guildId, new Set([userA])]]),
  };

  beforeAll(async () => {
    ctx = await createTestContext(permissions);
  });

  afterAll(async () => {
    await destroyTestContext(ctx);
  });

  beforeEach(async () => {
    await ctx.cleanup();
  });

  describe('User Settings Access', () => {
    it('user can access their own settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
        headers: { authorization: `Bearer ${token}` },
        query: { userId: userA },
      });

      expect(response.statusCode).toBe(200);
    });

    it('user cannot access another user\'s settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
        headers: { authorization: `Bearer ${token}` },
        query: { userId: userB },
      });

      expect(response.statusCode).toBe(403);
    });

    it('user can set their own user-scope settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'test',
          scopeType: 'user',
          userId: userA,
        },
      });

      expect(response.statusCode).toBe(204);
    });

    it('user cannot set another user\'s settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'test',
          scopeType: 'user',
          userId: userB,
        },
      });

      expect(response.statusCode).toBe(403);
    });

    it('user cannot set global scope settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'test',
          scopeType: 'global',
        },
      });

      expect(response.statusCode).toBe(403);
    });
  });

  describe('Guild Settings Access', () => {
    it('guild admin can access guild settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/guild.voice.following-rule',
        headers: { authorization: `Bearer ${token}` },
        query: { guildId },
      });

      expect(response.statusCode).toBe(200);
    });

    it('non-guild-admin cannot access guild settings', async () => {
      const token = await ctx.generateToken(userB);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/guild.voice.following-rule',
        headers: { authorization: `Bearer ${token}` },
        query: { guildId },
      });

      expect(response.statusCode).toBe(403);
    });

    it('guild admin can set guild settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/guild.voice.following-rule',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: { kind: 'followNonEmpty' },
          scopeType: 'guild',
          guildId,
        },
      });

      expect(response.statusCode).toBe(204);
    });

    it('non-guild-admin cannot set guild settings', async () => {
      const token = await ctx.generateToken(userB);

      const response = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/guild.voice.following-rule',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: { kind: 'followNonEmpty' },
          scopeType: 'guild',
          guildId,
        },
      });

      expect(response.statusCode).toBe(403);
    });

    it('guild admin can set user guild-scope settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'guild-default',
          scopeType: 'guild',
          guildId,
        },
      });

      expect(response.statusCode).toBe(204);
    });
  });

  describe('Admin Settings Access', () => {
    it('regular user cannot access admin settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/admin.access.admins',
        headers: { authorization: `Bearer ${token}` },
      });

      expect(response.statusCode).toBe(403);
    });

    it('bot admin can access admin settings', async () => {
      const token = await ctx.generateToken(botAdmin);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/admin.access.admins',
        headers: { authorization: `Bearer ${token}` },
      });

      expect(response.statusCode).toBe(200);
    });

    it('regular user cannot set admin settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/admin.access.admins',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: [userA],
          scopeType: 'admin',
        },
      });

      expect(response.statusCode).toBe(403);
    });

    it('bot admin can set admin settings', async () => {
      const token = await ctx.generateToken(botAdmin);

      const response = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/admin.access.admins',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: [botAdmin, userA],
          scopeType: 'admin',
        },
      });

      expect(response.statusCode).toBe(204);
    });
  });

  describe('Bot Admin Override', () => {
    it('bot admin can access any user\'s settings', async () => {
      const token = await ctx.generateToken(botAdmin);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
        headers: { authorization: `Bearer ${token}` },
        query: { userId: userA },
      });

      expect(response.statusCode).toBe(200);
    });

    it('bot admin can modify any user\'s settings', async () => {
      const token = await ctx.generateToken(botAdmin);

      const response = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'admin-set',
          scopeType: 'user',
          userId: userA,
        },
      });

      expect(response.statusCode).toBe(204);
    });

    it('bot admin can set global scope settings', async () => {
      const token = await ctx.generateToken(botAdmin);

      const response = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/guild.voice.following-rule',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: { kind: 'manual' },
          scopeType: 'global',
        },
      });

      expect(response.statusCode).toBe(204);
    });
  });

  describe('List Endpoint Authorization', () => {
    it('user can list their own settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings',
        headers: { authorization: `Bearer ${token}` },
        query: {
          settingsKind: 'user',
          userId: userA,
        },
      });

      expect(response.statusCode).toBe(200);
    });

    it('user cannot list another user\'s settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings',
        headers: { authorization: `Bearer ${token}` },
        query: {
          settingsKind: 'user',
          userId: userB,
        },
      });

      expect(response.statusCode).toBe(403);
    });

    it('guild admin can list guild settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings',
        headers: { authorization: `Bearer ${token}` },
        query: {
          settingsKind: 'guild',
          guildId,
        },
      });

      expect(response.statusCode).toBe(200);
    });

    it('regular user cannot list admin settings', async () => {
      const token = await ctx.generateToken(userA);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings',
        headers: { authorization: `Bearer ${token}` },
        query: { settingsKind: 'admin' },
      });

      expect(response.statusCode).toBe(403);
    });

    it('bot admin can list admin settings', async () => {
      const token = await ctx.generateToken(botAdmin);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings',
        headers: { authorization: `Bearer ${token}` },
        query: { settingsKind: 'admin' },
      });

      expect(response.statusCode).toBe(200);
    });
  });

  describe('Token Validation', () => {
    it('rejects invalid token', async () => {
      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
        headers: { authorization: 'Bearer invalid.token.here' },
      });

      expect(response.statusCode).toBe(401);
    });

    it('rejects expired token', async () => {
      const token = await ctx.generateToken(userA, { expiresIn: '-1h' });

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
        headers: { authorization: `Bearer ${token}` },
      });

      expect(response.statusCode).toBe(401);
    });

    it('rejects malformed authorization header', async () => {
      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
        headers: { authorization: 'NotBearer token' },
      });

      expect(response.statusCode).toBe(401);
    });
  });
});
