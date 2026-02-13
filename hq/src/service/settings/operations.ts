import { z } from 'zod';
import {
  type UserId,
  type GuildId,
  type Result,
  type ResolvedValue,
  type Scope,
  type SettingsKind,
  type AnySettingsKeyDefinition,
  type KeyRegistry,
  UserId as createUserId,
  GuildId as createGuildId,
  KeyIdentifier as createKeyIdentifier,
  ok,
  err,
  userScopeGlobal,
  userScopeGuild,
  userScopeUser,
  userScopePerGuildUser,
  guildScopeGlobal,
  guildScopeGuild,
  adminScopeAdmin,
  userContext,
  guildContext,
  adminContext,
  actorAdmin,
  actorGuildAdmin,
  actorUser,
} from 'zako3-settings';
import type { SettingsService } from './service.js';
import type {
  AuthContext,
  IGuildPermissionChecker,
  IBotAdminChecker,
} from '../../auth/index.js';

export const ScopeTypeSchema = z.enum([
  'global',
  'guild',
  'user',
  'perGuildUser',
  'admin',
]);
export type ScopeType = z.infer<typeof ScopeTypeSchema>;

export const SettingsKindSchema = z.enum(['user', 'guild', 'admin']);

export const GetSettingParamsSchema = z.object({
  keyId: z.string().min(1),
  userId: z.string().optional(),
  guildId: z.string().optional(),
});
export type GetSettingParams = z.infer<typeof GetSettingParamsSchema>;

export const SetSettingParamsSchema = z.object({
  keyId: z.string().min(1),
  value: z.unknown(),
  scopeType: ScopeTypeSchema,
  userId: z.string().optional(),
  guildId: z.string().optional(),
});
export type SetSettingParams = z.infer<typeof SetSettingParamsSchema>;

export const DeleteSettingParamsSchema = z.object({
  keyId: z.string().min(1),
  scopeType: ScopeTypeSchema,
  userId: z.string().optional(),
  guildId: z.string().optional(),
});
export type DeleteSettingParams = z.infer<typeof DeleteSettingParamsSchema>;

export const ListSettingsParamsSchema = z.object({
  settingsKind: SettingsKindSchema,
  scopeType: ScopeTypeSchema.optional(),
  userId: z.string().optional(),
  guildId: z.string().optional(),
});
export type ListSettingsParams = z.infer<typeof ListSettingsParamsSchema>;

export interface SettingEntry {
  keyId: string;
  value: unknown;
  scope: {
    kind: SettingsKind;
    type: string;
    guildId?: string;
    userId?: string;
  };
  isDefault: boolean;
}

export interface OperationsConfig {
  settingsService: SettingsService;
  guildChecker: IGuildPermissionChecker;
  adminChecker: IBotAdminChecker;
}

export interface SettingsOperations {
  get(
    params: GetSettingParams,
    auth: AuthContext
  ): Promise<Result<ResolvedValue<unknown>>>;

  set(params: SetSettingParams, auth: AuthContext): Promise<Result<void>>;

  delete(
    params: DeleteSettingParams,
    auth: AuthContext
  ): Promise<Result<boolean>>;

  list(
    params: ListSettingsParams,
    auth: AuthContext
  ): Promise<Result<SettingEntry[]>>;
}

export const OperationErrors = {
  INVALID_PARAMS: 'Invalid parameters',
  KEY_NOT_FOUND: 'Setting key not found',
  INVALID_SCOPE: 'Invalid scope configuration',
  FORBIDDEN: 'Access denied',
  INTERNAL_ERROR: 'Internal error',
} as const;

function getKeyKind(key: AnySettingsKeyDefinition): SettingsKind {
  return key.settingsKind;
}

function buildScope(
  kind: SettingsKind,
  scopeType: ScopeType,
  userId?: string,
  guildId?: string
): Result<Scope> {
  switch (kind) {
    case 'user': {
      switch (scopeType) {
        case 'global':
          return ok(userScopeGlobal());
        case 'guild':
          if (!guildId) return err('guildId required for guild scope');
          return ok(userScopeGuild(createGuildId(guildId)));
        case 'user':
          if (!userId) return err('userId required for user scope');
          return ok(userScopeUser(createUserId(userId)));
        case 'perGuildUser':
          if (!guildId || !userId)
            return err('guildId and userId required for perGuildUser scope');
          return ok(
            userScopePerGuildUser(createGuildId(guildId), createUserId(userId))
          );
        default:
          return err(`Invalid scope type ${scopeType} for user settings`);
      }
    }
    case 'guild': {
      switch (scopeType) {
        case 'global':
          return ok(guildScopeGlobal());
        case 'guild':
          if (!guildId) return err('guildId required for guild scope');
          return ok(guildScopeGuild(createGuildId(guildId)));
        default:
          return err(`Invalid scope type ${scopeType} for guild settings`);
      }
    }
    case 'admin': {
      if (scopeType !== 'admin') {
        return err(`Invalid scope type ${scopeType} for admin settings`);
      }
      return ok(adminScopeAdmin());
    }
  }
}

function getSourceScope(
  resolved: ResolvedValue<unknown>
): { kind: SettingsKind; type: string; guildId?: string; userId?: string } | null {
  if (resolved.source.kind === 'default') {
    return null;
  }
  if (resolved.source.kind === 'entry') {
    const scope = resolved.source.scope;
    return {
      kind: scope.kind,
      type: scope.scope,
      guildId: 'guildId' in scope ? (scope.guildId as string) : undefined,
      userId: 'userId' in scope ? (scope.userId as string) : undefined,
    };
  }
  return null;
}

function isDefaultValue(resolved: ResolvedValue<unknown>): boolean {
  return resolved.source.kind === 'default';
}

export function createSettingsOperations(
  config: OperationsConfig
): SettingsOperations {
  const { settingsService, guildChecker, adminChecker } = config;

  // Use the registry from the settings service manager
  const registry = settingsService.manager.registry;

  function findKeyDefinition(keyId: string): AnySettingsKeyDefinition | undefined {
    return registry.get(createKeyIdentifier(keyId));
  }

  async function checkReadPermission(
    auth: AuthContext,
    kind: SettingsKind,
    targetUserId?: string,
    guildId?: string
  ): Promise<Result<void>> {
    const isBotAdmin = await adminChecker.isBotAdmin(auth.userId);

    if (isBotAdmin) {
      return ok(undefined);
    }

    switch (kind) {
      case 'user': {
        if (targetUserId && targetUserId === (auth.userId as string)) {
          return ok(undefined);
        }
        if (guildId) {
          const isGuildAdmin = await guildChecker.isGuildAdmin(
            auth.userId,
            createGuildId(guildId)
          );
          if (isGuildAdmin) {
            return ok(undefined);
          }
        }
        if (targetUserId && targetUserId !== (auth.userId as string)) {
          return err(OperationErrors.FORBIDDEN);
        }
        return ok(undefined);
      }
      case 'guild': {
        if (!guildId) {
          return err(OperationErrors.FORBIDDEN);
        }
        const isGuildAdmin = await guildChecker.isGuildAdmin(
          auth.userId,
          createGuildId(guildId)
        );
        if (!isGuildAdmin) {
          return err(OperationErrors.FORBIDDEN);
        }
        return ok(undefined);
      }
      case 'admin': {
        return err(OperationErrors.FORBIDDEN);
      }
    }
  }

  async function checkWritePermission(
    auth: AuthContext,
    kind: SettingsKind,
    scopeType: ScopeType,
    targetUserId?: string,
    guildId?: string
  ): Promise<Result<void>> {
    const isBotAdmin = await adminChecker.isBotAdmin(auth.userId);

    if (isBotAdmin) {
      return ok(undefined);
    }

    if (scopeType === 'global') {
      return err(OperationErrors.FORBIDDEN);
    }

    switch (kind) {
      case 'user': {
        switch (scopeType) {
          case 'user':
            if (targetUserId === (auth.userId as string)) {
              return ok(undefined);
            }
            return err(OperationErrors.FORBIDDEN);
          case 'guild':
            if (!guildId) return err(OperationErrors.INVALID_SCOPE);
            const isGuildAdminForGuild = await guildChecker.isGuildAdmin(
              auth.userId,
              createGuildId(guildId)
            );
            if (!isGuildAdminForGuild) {
              return err(OperationErrors.FORBIDDEN);
            }
            return ok(undefined);
          case 'perGuildUser':
            if (targetUserId === (auth.userId as string)) {
              return ok(undefined);
            }
            if (guildId) {
              const isAdmin = await guildChecker.isGuildAdmin(
                auth.userId,
                createGuildId(guildId)
              );
              if (isAdmin) {
                return ok(undefined);
              }
            }
            return err(OperationErrors.FORBIDDEN);
          default:
            return err(OperationErrors.INVALID_SCOPE);
        }
      }
      case 'guild': {
        if (!guildId) {
          return err(OperationErrors.INVALID_SCOPE);
        }
        const isGuildAdmin = await guildChecker.isGuildAdmin(
          auth.userId,
          createGuildId(guildId)
        );
        if (!isGuildAdmin) {
          return err(OperationErrors.FORBIDDEN);
        }
        return ok(undefined);
      }
      case 'admin': {
        return err(OperationErrors.FORBIDDEN);
      }
    }
  }

  async function checkListPermission(
    auth: AuthContext,
    kind: SettingsKind,
    targetUserId?: string,
    guildId?: string
  ): Promise<Result<void>> {
    const isBotAdmin = await adminChecker.isBotAdmin(auth.userId);

    if (isBotAdmin) {
      return ok(undefined);
    }

    switch (kind) {
      case 'user': {
        if (targetUserId === (auth.userId as string)) {
          return ok(undefined);
        }
        return err(OperationErrors.FORBIDDEN);
      }
      case 'guild': {
        if (!guildId) {
          return err(OperationErrors.FORBIDDEN);
        }
        const isGuildAdmin = await guildChecker.isGuildAdmin(
          auth.userId,
          createGuildId(guildId)
        );
        if (!isGuildAdmin) {
          return err(OperationErrors.FORBIDDEN);
        }
        return ok(undefined);
      }
      case 'admin': {
        return err(OperationErrors.FORBIDDEN);
      }
    }
  }

  return {
    async get(
      params: GetSettingParams,
      auth: AuthContext
    ): Promise<Result<ResolvedValue<unknown>>> {
      const validation = GetSettingParamsSchema.safeParse(params);
      if (!validation.success) {
        return err(OperationErrors.INVALID_PARAMS);
      }

      const { keyId, userId, guildId } = validation.data;
      const key = findKeyDefinition(keyId);
      if (!key) {
        return err(OperationErrors.KEY_NOT_FOUND);
      }

      const kind = getKeyKind(key);
      const permissionCheck = await checkReadPermission(
        auth,
        kind,
        userId,
        guildId
      );
      if (!permissionCheck.ok) {
        return err(permissionCheck.error);
      }

      let context;
      switch (kind) {
        case 'user':
          context = userContext(
            userId ? createUserId(userId) : auth.userId,
            guildId ? createGuildId(guildId) : undefined
          );
          break;
        case 'guild':
          if (!guildId) {
            return err('guildId required for guild settings');
          }
          context = guildContext(createGuildId(guildId));
          break;
        case 'admin':
          context = adminContext();
          break;
      }

      return settingsService.get(key, context);
    },

    async set(params: SetSettingParams, auth: AuthContext): Promise<Result<void>> {
      const validation = SetSettingParamsSchema.safeParse(params);
      if (!validation.success) {
        return err(OperationErrors.INVALID_PARAMS);
      }

      const { keyId, value, scopeType, userId, guildId } = validation.data;
      const key = findKeyDefinition(keyId);
      if (!key) {
        return err(OperationErrors.KEY_NOT_FOUND);
      }

      const kind = getKeyKind(key);
      const permissionCheck = await checkWritePermission(
        auth,
        kind,
        scopeType,
        userId,
        guildId
      );
      if (!permissionCheck.ok) {
        return err(permissionCheck.error);
      }

      const scopeResult = buildScope(kind, scopeType, userId, guildId);
      if (!scopeResult.ok) {
        return err(scopeResult.error);
      }

      const isBotAdmin = await adminChecker.isBotAdmin(auth.userId);
      let actor;
      if (isBotAdmin) {
        actor = actorAdmin();
      } else if (guildId) {
        const isGuildAdmin = await guildChecker.isGuildAdmin(
          auth.userId,
          createGuildId(guildId)
        );
        if (isGuildAdmin) {
          actor = actorGuildAdmin(createGuildId(guildId));
        } else {
          actor = actorUser(auth.userId);
        }
      } else {
        actor = actorUser(auth.userId);
      }

      return settingsService.set(key, value, scopeResult.value, actor);
    },

    async delete(
      params: DeleteSettingParams,
      auth: AuthContext
    ): Promise<Result<boolean>> {
      const validation = DeleteSettingParamsSchema.safeParse(params);
      if (!validation.success) {
        return err(OperationErrors.INVALID_PARAMS);
      }

      const { keyId, scopeType, userId, guildId } = validation.data;
      const key = findKeyDefinition(keyId);
      if (!key) {
        return err(OperationErrors.KEY_NOT_FOUND);
      }

      const kind = getKeyKind(key);
      const permissionCheck = await checkWritePermission(
        auth,
        kind,
        scopeType,
        userId,
        guildId
      );
      if (!permissionCheck.ok) {
        return err(permissionCheck.error);
      }

      const scopeResult = buildScope(kind, scopeType, userId, guildId);
      if (!scopeResult.ok) {
        return err(scopeResult.error);
      }

      const isBotAdmin = await adminChecker.isBotAdmin(auth.userId);
      let actor;
      if (isBotAdmin) {
        actor = actorAdmin();
      } else if (guildId) {
        const isGuildAdmin = await guildChecker.isGuildAdmin(
          auth.userId,
          createGuildId(guildId)
        );
        if (isGuildAdmin) {
          actor = actorGuildAdmin(createGuildId(guildId));
        } else {
          actor = actorUser(auth.userId);
        }
      } else {
        actor = actorUser(auth.userId);
      }

      return settingsService.delete(key, scopeResult.value, actor);
    },

    async list(
      params: ListSettingsParams,
      auth: AuthContext
    ): Promise<Result<SettingEntry[]>> {
      const validation = ListSettingsParamsSchema.safeParse(params);
      if (!validation.success) {
        return err(OperationErrors.INVALID_PARAMS);
      }

      const { settingsKind, scopeType, userId, guildId } = validation.data;

      const permissionCheck = await checkListPermission(
        auth,
        settingsKind,
        userId,
        guildId
      );
      if (!permissionCheck.ok) {
        return err(permissionCheck.error);
      }

      const allKeys = registry.getAll();

      const relevantKeys = allKeys.filter(
        (key) => getKeyKind(key) === settingsKind
      );

      const entries: SettingEntry[] = [];

      for (const key of relevantKeys) {
        const kind = getKeyKind(key);
        let context;
        switch (kind) {
          case 'user':
            context = userContext(
              userId ? createUserId(userId) : auth.userId,
              guildId ? createGuildId(guildId) : undefined
            );
            break;
          case 'guild':
            if (!guildId) continue;
            context = guildContext(createGuildId(guildId));
            break;
          case 'admin':
            context = adminContext();
            break;
        }

        const result = await settingsService.get(key, context);
        if (result.ok) {
          const resolved = result.value;
          const sourceScope = getSourceScope(resolved);
          const isDefault = isDefaultValue(resolved);

          if (scopeType && sourceScope && sourceScope.type !== scopeType) {
            continue;
          }

          entries.push({
            keyId: key.identifier as string,
            value: resolved.value,
            scope: sourceScope ?? {
              kind: kind,
              type: 'default',
            },
            isDefault,
          });
        }
      }

      return ok(entries);
    },
  };
}
