import { describe, it, expect, beforeAll, afterAll, beforeEach } from 'vitest';
import { createTestContext, destroyTestContext, type TestContext, type MockPermissions } from './helpers.js';

describe('Settings CRUD', () => {
  let ctx: TestContext;
  const testUserId = '123456789';
  const testGuildId = '987654321';

  const permissions: MockPermissions = {
    botAdmins: new Set(),
    guildAdmins: new Map(),
  };

  beforeAll(async () => {
    permissions.guildAdmins.set(testGuildId, new Set([testUserId]));
    ctx = await createTestContext(permissions);
  });

  afterAll(async () => {
    await destroyTestContext(ctx);
  });

  beforeEach(async () => {
    await ctx.cleanup();
  });

  describe('GET /api/v1/settings/:keyId', () => {
    it('returns 401 without authentication', async () => {
      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
      });

      expect(response.statusCode).toBe(401);
    });

    it('returns default value for unset setting', async () => {
      const token = await ctx.generateToken(testUserId);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
        },
        query: {
          userId: testUserId,
        },
      });

      expect(response.statusCode).toBe(200);
      const body = response.json();
      expect(body.value).toBeDefined();
      expect(body.source.kind).toBe('default');
    });

    it('returns 404 for unknown key', async () => {
      const token = await ctx.generateToken(testUserId);

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/nonexistent.key.id',
        headers: {
          authorization: `Bearer ${token}`,
        },
      });

      expect(response.statusCode).toBe(404);
    });
  });

  describe('PUT /api/v1/settings/:keyId', () => {
    it('sets a user setting at user scope', async () => {
      const token = await ctx.generateToken(testUserId);

      const setResponse = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'azure',
          scopeType: 'user',
          userId: testUserId,
        },
      });

      expect(setResponse.statusCode).toBe(204);

      const getResponse = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
        },
        query: {
          userId: testUserId,
        },
      });

      expect(getResponse.statusCode).toBe(200);
      const body = getResponse.json();
      expect(body.value).toBe('azure');
      expect(body.source.kind).toBe('entry');
    });

    it('sets a user setting at perGuildUser scope', async () => {
      const token = await ctx.generateToken(testUserId);

      const setResponse = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'google',
          scopeType: 'perGuildUser',
          userId: testUserId,
          guildId: testGuildId,
        },
      });

      expect(setResponse.statusCode).toBe(204);

      const getResponse = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
        },
        query: {
          userId: testUserId,
          guildId: testGuildId,
        },
      });

      expect(getResponse.statusCode).toBe(200);
      const body = getResponse.json();
      expect(body.value).toBe('google');
    });

    it('returns 400 for invalid scope configuration', async () => {
      const token = await ctx.generateToken(testUserId);

      const response = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'test',
          scopeType: 'guild',
        },
      });

      expect(response.statusCode).toBe(400);
    });

    it('returns 404 for unknown key', async () => {
      const token = await ctx.generateToken(testUserId);

      const response = await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/nonexistent.key.id',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'test',
          scopeType: 'user',
          userId: testUserId,
        },
      });

      expect(response.statusCode).toBe(404);
    });
  });

  describe('DELETE /api/v1/settings/:keyId', () => {
    it('deletes an existing setting', async () => {
      const token = await ctx.generateToken(testUserId);

      await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'azure',
          scopeType: 'user',
          userId: testUserId,
        },
      });

      const deleteResponse = await ctx.app.inject({
        method: 'DELETE',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
        },
        query: {
          scopeType: 'user',
          userId: testUserId,
        },
      });

      expect(deleteResponse.statusCode).toBe(200);
      expect(deleteResponse.json().deleted).toBe(true);

      const getResponse = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
        },
        query: {
          userId: testUserId,
        },
      });

      expect(getResponse.statusCode).toBe(200);
      expect(getResponse.json().source.kind).toBe('default');
    });

    it('returns deleted:false for non-existent setting', async () => {
      const token = await ctx.generateToken(testUserId);

      const response = await ctx.app.inject({
        method: 'DELETE',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
        },
        query: {
          scopeType: 'user',
          userId: testUserId,
        },
      });

      expect(response.statusCode).toBe(200);
      expect(response.json().deleted).toBe(false);
    });
  });

  describe('GET /api/v1/settings (list)', () => {
    it('lists user settings for authenticated user', async () => {
      const token = await ctx.generateToken(testUserId);

      await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'azure',
          scopeType: 'user',
          userId: testUserId,
        },
      });

      const response = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings',
        headers: {
          authorization: `Bearer ${token}`,
        },
        query: {
          settingsKind: 'user',
          userId: testUserId,
        },
      });

      expect(response.statusCode).toBe(200);
      const body = response.json();
      expect(body.entries).toBeInstanceOf(Array);
      expect(body.entries.length).toBeGreaterThan(0);

      const ttsVoice = body.entries.find(
        (e: { keyId: string }) => e.keyId === 'user.tts.voice'
      );
      expect(ttsVoice).toBeDefined();
      expect(ttsVoice.value).toBe('azure');
    });

    it('filters by scope type', async () => {
      const token = await ctx.generateToken(testUserId);

      await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'azure',
          scopeType: 'user',
          userId: testUserId,
        },
      });

      const responseUser = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings',
        headers: {
          authorization: `Bearer ${token}`,
        },
        query: {
          settingsKind: 'user',
          scopeType: 'user',
          userId: testUserId,
        },
      });

      expect(responseUser.statusCode).toBe(200);
      const userEntries = responseUser.json().entries;
      const ttsVoice = userEntries.find(
        (e: { keyId: string }) => e.keyId === 'user.tts.voice'
      );
      expect(ttsVoice).toBeDefined();

      const responseGuild = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings',
        headers: {
          authorization: `Bearer ${token}`,
        },
        query: {
          settingsKind: 'user',
          scopeType: 'guild',
          userId: testUserId,
        },
      });

      expect(responseGuild.statusCode).toBe(200);
      const guildEntries = responseGuild.json().entries;
      const ttsVoiceGuild = guildEntries.find(
        (e: { keyId: string }) => e.keyId === 'user.tts.voice'
      );
      expect(ttsVoiceGuild).toBeUndefined();
    });
  });

  describe('Scope Cascading', () => {
    it('perGuildUser overrides user scope', async () => {
      const token = await ctx.generateToken(testUserId);

      await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'user-level',
          scopeType: 'user',
          userId: testUserId,
        },
      });

      await ctx.app.inject({
        method: 'PUT',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
          'content-type': 'application/json',
        },
        payload: {
          value: 'per-guild-user-level',
          scopeType: 'perGuildUser',
          userId: testUserId,
          guildId: testGuildId,
        },
      });

      const withGuildResponse = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
        },
        query: {
          userId: testUserId,
          guildId: testGuildId,
        },
      });

      expect(withGuildResponse.statusCode).toBe(200);
      expect(withGuildResponse.json().value).toBe('per-guild-user-level');

      const withoutGuildResponse = await ctx.app.inject({
        method: 'GET',
        url: '/api/v1/settings/user.tts.voice',
        headers: {
          authorization: `Bearer ${token}`,
        },
        query: {
          userId: testUserId,
        },
      });

      expect(withoutGuildResponse.statusCode).toBe(200);
      expect(withoutGuildResponse.json().value).toBe('user-level');
    });
  });
});
