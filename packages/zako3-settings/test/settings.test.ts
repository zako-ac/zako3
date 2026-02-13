import { describe, it, expect } from 'vitest';
import {
  UserId,
  GuildId,
  KeyIdentifier,
  TapRef,
  ok,
  err,
  isOk,
  isErr,
  unwrap,
  userScopeGlobal,
  userScopeUser,
  userScopeGuild,
  userScopePerGuildUser,
  USER_KEY_TTS_VOICE,
  createKeyRegistry,
  createInMemoryAdapter,
  createSettingsManager,
  userContext,
} from '../src';

describe('Branded Types', () => {
  it('creates UserId correctly', () => {
    const userId = UserId('123456789');
    expect(userId).toBe('123456789');
  });

  it('creates GuildId correctly', () => {
    const guildId = GuildId('987654321');
    expect(guildId).toBe('987654321');
  });

  it('creates KeyIdentifier correctly', () => {
    const keyId = KeyIdentifier('user.tts.voice');
    expect(keyId).toBe('user.tts.voice');
  });

  it('creates TapRef correctly', () => {
    const tapRef = TapRef('azure');
    expect(tapRef).toBe('azure');
  });
});

describe('Result Type', () => {
  it('ok() creates a successful result', () => {
    const result = ok(42);
    expect(isOk(result)).toBe(true);
    expect(isErr(result)).toBe(false);
    expect(unwrap(result)).toBe(42);
  });

  it('err() creates an error result', () => {
    const result = err('something went wrong');
    expect(isOk(result)).toBe(false);
    expect(isErr(result)).toBe(true);
  });
});

describe('Scopes', () => {
  it('creates user scopes correctly', () => {
    const global = userScopeGlobal();
    expect(global.kind).toBe('user');
    expect(global.scope).toBe('global');

    const userId = UserId('123');
    const user = userScopeUser(userId);
    expect(user.kind).toBe('user');
    expect(user.scope).toBe('user');
    expect(user.userId).toBe(userId);

    const guildId = GuildId('456');
    const guild = userScopeGuild(guildId);
    expect(guild.kind).toBe('user');
    expect(guild.scope).toBe('guild');
    expect(guild.guildId).toBe(guildId);

    const perGuildUser = userScopePerGuildUser(guildId, userId);
    expect(perGuildUser.kind).toBe('user');
    expect(perGuildUser.scope).toBe('perGuildUser');
    expect(perGuildUser.guildId).toBe(guildId);
    expect(perGuildUser.userId).toBe(userId);
  });
});

describe('Key Registry', () => {
  it('registers and retrieves keys', () => {
    const registry = createKeyRegistry();
    registry.register(USER_KEY_TTS_VOICE);

    const retrieved = registry.get(USER_KEY_TTS_VOICE.identifier);
    expect(retrieved).toBeDefined();
    expect(retrieved?.identifier).toBe(USER_KEY_TTS_VOICE.identifier);
  });

  it('returns undefined for unknown keys', () => {
    const registry = createKeyRegistry();
    const result = registry.get(KeyIdentifier('unknown.key'));
    expect(result).toBeUndefined();
  });
});

describe('Settings Manager', () => {
  it('creates and initializes successfully', async () => {
    const adapter = createInMemoryAdapter();
    const manager = createSettingsManager({
      persistence: adapter,
      keys: [USER_KEY_TTS_VOICE],
    });

    await manager.initialize();

    // Getting a value should return the default when nothing is set
    const result = await manager.get(
      USER_KEY_TTS_VOICE,
      userContext(UserId('123'), GuildId('456'))
    );

    expect(isOk(result)).toBe(true);
    if (isOk(result)) {
      // source is an object with kind property
      expect(result.value.source.kind).toBe('default');
    }
  });
});
