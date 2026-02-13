/**
 * @fileoverview Special/complex value types for settings.
 *
 * This module defines domain-specific value types used in the settings system,
 * including enum-like types and complex composite types.
 */

import { type Result, ok, err } from './result';
import type { ValueTypeDescriptor } from './primitives';
import { type TapRef, TapRef as createTapRef, type EmojiId, type StickerId } from './identifiers';

// =============================================================================
// Enum Description Trait
// =============================================================================

/**
 * Interface for enum-like types that can provide descriptions for each variant.
 */
export interface EnumDescribable {
  /** Returns a human-readable description of each variant */
  getVariantDescriptions(): ReadonlyMap<string, string>;
}

// =============================================================================
// VoiceChannelFollowingRule
// =============================================================================

/**
 * Voice channel following behavior options.
 */
export type VoiceChannelFollowingRule =
  | { readonly kind: 'manual' }
  | { readonly kind: 'followNonEmpty' };

/**
 * Creates a "manual" voice channel following rule.
 */
export function vcFollowManual(): VoiceChannelFollowingRule {
  return { kind: 'manual' };
}

/**
 * Creates a "follow non-empty channel" voice channel following rule.
 */
export function vcFollowNonEmpty(): VoiceChannelFollowingRule {
  return { kind: 'followNonEmpty' };
}

/**
 * Descriptions for VoiceChannelFollowingRule variants.
 */
export const VOICE_CHANNEL_FOLLOWING_RULE_DESCRIPTIONS: ReadonlyMap<string, string> = new Map([
  ['manual', 'Users control the bot\'s channel by command.'],
  ['followNonEmpty', 'Follow if there\'s a non-empty voice channel, if the bot is not currently being used in the guild.'],
]);

/**
 * Type descriptor for VoiceChannelFollowingRule.
 */
export interface VoiceChannelFollowingRuleTypeDescriptor
  extends ValueTypeDescriptor<VoiceChannelFollowingRule>,
    EnumDescribable {
  readonly kind: 'voiceChannelFollowingRule';
}

/**
 * Creates a VoiceChannelFollowingRule type descriptor.
 */
export function voiceChannelFollowingRuleType(
  defaultValue: VoiceChannelFollowingRule = vcFollowManual()
): VoiceChannelFollowingRuleTypeDescriptor {
  return {
    kind: 'voiceChannelFollowingRule',
    displayName: 'Voice Channel Following Rule',

    validate(value: unknown): Result<VoiceChannelFollowingRule> {
      if (typeof value !== 'object' || value === null) {
        return err(`Expected object with kind property, got ${typeof value}`);
      }

      const obj = value as Record<string, unknown>;

      if (obj.kind === 'manual') {
        return ok(vcFollowManual());
      }

      if (obj.kind === 'followNonEmpty') {
        return ok(vcFollowNonEmpty());
      }

      return err(`Invalid kind '${obj.kind}'. Expected 'manual' or 'followNonEmpty'.`);
    },

    getDefault(): VoiceChannelFollowingRule {
      return defaultValue;
    },

    serialize(value: VoiceChannelFollowingRule): unknown {
      return { kind: value.kind };
    },

    deserialize(data: unknown): Result<VoiceChannelFollowingRule> {
      return this.validate(data);
    },

    getVariantDescriptions(): ReadonlyMap<string, string> {
      return VOICE_CHANNEL_FOLLOWING_RULE_DESCRIPTIONS;
    },
  };
}

// =============================================================================
// MemberFilter
// =============================================================================

/**
 * Discord permission flags (subset commonly used).
 * These match Discord's permission bit flags.
 */
export const PermissionFlags = {
  ADMINISTRATOR: 1n << 3n,
  MANAGE_CHANNELS: 1n << 4n,
  MANAGE_GUILD: 1n << 5n,
  MANAGE_MESSAGES: 1n << 13n,
  MANAGE_ROLES: 1n << 28n,
  MODERATE_MEMBERS: 1n << 40n,
} as const;

export type PermissionFlag = bigint;

/**
 * Member filter for permission-based access control.
 */
export type MemberFilter =
  | { readonly kind: 'anyone' }
  | { readonly kind: 'withPermission'; readonly permissions: readonly PermissionFlag[] };

/**
 * Creates an "anyone" member filter.
 */
export function memberFilterAnyone(): MemberFilter {
  return { kind: 'anyone' };
}

/**
 * Creates a "with permission" member filter.
 */
export function memberFilterWithPermission(
  permissions: readonly PermissionFlag[]
): MemberFilter {
  return { kind: 'withPermission', permissions };
}

/**
 * Descriptions for MemberFilter variants.
 */
export const MEMBER_FILTER_DESCRIPTIONS: ReadonlyMap<string, string> = new Map([
  ['anyone', 'Anyone can use this feature.'],
  ['withPermission', 'Only members with specific permissions can use this feature.'],
]);

/**
 * Type descriptor for MemberFilter.
 */
export interface MemberFilterTypeDescriptor
  extends ValueTypeDescriptor<MemberFilter>,
    EnumDescribable {
  readonly kind: 'memberFilter';
}

/**
 * Creates a MemberFilter type descriptor.
 */
export function memberFilterType(
  defaultValue: MemberFilter = memberFilterAnyone()
): MemberFilterTypeDescriptor {
  return {
    kind: 'memberFilter',
    displayName: 'Member Filter',

    validate(value: unknown): Result<MemberFilter> {
      if (typeof value !== 'object' || value === null) {
        return err(`Expected object with kind property, got ${typeof value}`);
      }

      const obj = value as Record<string, unknown>;

      if (obj.kind === 'anyone') {
        return ok(memberFilterAnyone());
      }

      if (obj.kind === 'withPermission') {
        if (!Array.isArray(obj.permissions)) {
          return err('withPermission requires a permissions array');
        }
        // Permissions stored as strings for JSON serialization, convert to bigint
        const permissions = obj.permissions.map((p) =>
          typeof p === 'string' ? BigInt(p) : BigInt(p as number)
        );
        return ok(memberFilterWithPermission(permissions));
      }

      return err(`Invalid kind '${obj.kind}'. Expected 'anyone' or 'withPermission'.`);
    },

    getDefault(): MemberFilter {
      return defaultValue;
    },

    serialize(value: MemberFilter): unknown {
      if (value.kind === 'anyone') {
        return { kind: 'anyone' };
      }
      // Serialize bigints as strings for JSON compatibility
      return {
        kind: 'withPermission',
        permissions: value.permissions.map((p) => p.toString()),
      };
    },

    deserialize(data: unknown): Result<MemberFilter> {
      return this.validate(data);
    },

    getVariantDescriptions(): ReadonlyMap<string, string> {
      return MEMBER_FILTER_DESCRIPTIONS;
    },
  };
}

// =============================================================================
// Text Mapping Types
// =============================================================================

/**
 * Simple text-to-text mapping.
 */
export interface SimpleTextMapping {
  readonly kind: 'simple';
  readonly from: string;
  readonly to: string;
}

/**
 * Regex-based text replacement mapping.
 */
export interface RegexTextMapping {
  readonly kind: 'regex';
  readonly fromRegex: string;
  readonly replaceTo: string;
}

/**
 * Union of text mapping types.
 */
export type TextMapping = SimpleTextMapping | RegexTextMapping;

/**
 * Creates a simple text mapping.
 */
export function simpleTextMapping(from: string, to: string): SimpleTextMapping {
  return { kind: 'simple', from, to };
}

/**
 * Creates a regex text mapping.
 */
export function regexTextMapping(fromRegex: string, replaceTo: string): RegexTextMapping {
  return { kind: 'regex', fromRegex, replaceTo };
}

// =============================================================================
// Emoji Mapping
// =============================================================================

/**
 * Emoji to text mapping (or disabled).
 */
export interface EmojiMapping {
  readonly emoji: EmojiId;
  readonly text: string | null; // null means "off"
}

/**
 * Creates an emoji mapping.
 */
export function emojiMapping(emoji: EmojiId, text: string | null): EmojiMapping {
  return { emoji, text };
}

// =============================================================================
// Sticker Mapping
// =============================================================================

/**
 * Sticker to text mapping (or disabled).
 */
export interface StickerMapping {
  readonly sticker: StickerId;
  readonly text: string | null; // null means "off"
}

/**
 * Creates a sticker mapping.
 */
export function stickerMapping(sticker: StickerId, text: string | null): StickerMapping {
  return { sticker, text };
}

// =============================================================================
// MappingConfig (Mergeable)
// =============================================================================

/**
 * Complete mapping configuration.
 * This type supports precedence merging.
 */
export interface MappingConfig {
  readonly textMappings: readonly TextMapping[];
  readonly emojiMappings: readonly EmojiMapping[];
  readonly stickerMappings: readonly StickerMapping[];
}

/**
 * Creates an empty mapping config.
 */
export function emptyMappingConfig(): MappingConfig {
  return {
    textMappings: [],
    emojiMappings: [],
    stickerMappings: [],
  };
}

/**
 * Creates a mapping config.
 */
export function mappingConfig(
  textMappings: readonly TextMapping[] = [],
  emojiMappings: readonly EmojiMapping[] = [],
  stickerMappings: readonly StickerMapping[] = []
): MappingConfig {
  return { textMappings, emojiMappings, stickerMappings };
}

/**
 * Merges two mapping configs by concatenating their arrays.
 * Later entries in `other` take precedence when applied.
 */
export function mergeMappingConfigs(base: MappingConfig, other: MappingConfig): MappingConfig {
  return {
    textMappings: [...base.textMappings, ...other.textMappings],
    emojiMappings: [...base.emojiMappings, ...other.emojiMappings],
    stickerMappings: [...base.stickerMappings, ...other.stickerMappings],
  };
}

/**
 * Type descriptor for MappingConfig.
 */
export interface MappingConfigTypeDescriptor extends ValueTypeDescriptor<MappingConfig> {
  readonly kind: 'mappingConfig';
  readonly isMergeable: true;
}

/**
 * Creates a MappingConfig type descriptor.
 */
export function mappingConfigType(): MappingConfigTypeDescriptor {
  return {
    kind: 'mappingConfig',
    displayName: 'Mapping Configuration',
    isMergeable: true,

    validate(value: unknown): Result<MappingConfig> {
      if (typeof value !== 'object' || value === null) {
        return err(`Expected object, got ${typeof value}`);
      }

      const obj = value as Record<string, unknown>;
      const textMappings: TextMapping[] = [];
      const emojiMappings: EmojiMapping[] = [];
      const stickerMappings: StickerMapping[] = [];

      // Validate text mappings
      if (obj.textMappings !== undefined) {
        if (!Array.isArray(obj.textMappings)) {
          return err('textMappings must be an array');
        }
        for (let i = 0; i < obj.textMappings.length; i++) {
          const tm = obj.textMappings[i] as Record<string, unknown>;
          if (tm.kind === 'simple') {
            if (typeof tm.from !== 'string' || typeof tm.to !== 'string') {
              return err(`Invalid simple text mapping at index ${i}`);
            }
            textMappings.push(simpleTextMapping(tm.from, tm.to));
          } else if (tm.kind === 'regex') {
            if (typeof tm.fromRegex !== 'string' || typeof tm.replaceTo !== 'string') {
              return err(`Invalid regex text mapping at index ${i}`);
            }
            // Validate regex is valid
            try {
              new RegExp(tm.fromRegex);
            } catch {
              return err(`Invalid regex pattern at index ${i}: ${tm.fromRegex}`);
            }
            textMappings.push(regexTextMapping(tm.fromRegex, tm.replaceTo));
          } else {
            return err(`Invalid text mapping kind at index ${i}`);
          }
        }
      }

      // Validate emoji mappings
      if (obj.emojiMappings !== undefined) {
        if (!Array.isArray(obj.emojiMappings)) {
          return err('emojiMappings must be an array');
        }
        for (let i = 0; i < obj.emojiMappings.length; i++) {
          const em = obj.emojiMappings[i] as Record<string, unknown>;
          if (typeof em.emoji !== 'string') {
            return err(`Invalid emoji at index ${i}`);
          }
          if (em.text !== null && typeof em.text !== 'string') {
            return err(`Invalid text for emoji at index ${i}`);
          }
          emojiMappings.push(
            emojiMapping(
              em.emoji as EmojiId,
              em.text as string | null
            )
          );
        }
      }

      // Validate sticker mappings
      if (obj.stickerMappings !== undefined) {
        if (!Array.isArray(obj.stickerMappings)) {
          return err('stickerMappings must be an array');
        }
        for (let i = 0; i < obj.stickerMappings.length; i++) {
          const sm = obj.stickerMappings[i] as Record<string, unknown>;
          if (typeof sm.sticker !== 'string') {
            return err(`Invalid sticker at index ${i}`);
          }
          if (sm.text !== null && typeof sm.text !== 'string') {
            return err(`Invalid text for sticker at index ${i}`);
          }
          stickerMappings.push(
            stickerMapping(
              sm.sticker as StickerId,
              sm.text as string | null
            )
          );
        }
      }

      return ok(mappingConfig(textMappings, emojiMappings, stickerMappings));
    },

    getDefault(): MappingConfig {
      return emptyMappingConfig();
    },

    serialize(value: MappingConfig): unknown {
      return {
        textMappings: value.textMappings.map((tm) => {
          if (tm.kind === 'simple') {
            return { kind: 'simple', from: tm.from, to: tm.to };
          }
          return { kind: 'regex', fromRegex: tm.fromRegex, replaceTo: tm.replaceTo };
        }),
        emojiMappings: value.emojiMappings.map((em) => ({
          emoji: em.emoji,
          text: em.text,
        })),
        stickerMappings: value.stickerMappings.map((sm) => ({
          sticker: sm.sticker,
          text: sm.text,
        })),
      };
    },

    deserialize(data: unknown): Result<MappingConfig> {
      return this.validate(data);
    },
  };
}

// =============================================================================
// TapRef Type Descriptor
// =============================================================================

/**
 * Type descriptor for TapRef.
 */
export interface TapRefTypeDescriptor extends ValueTypeDescriptor<TapRef> {
  readonly kind: 'tapRef';
}

/**
 * Creates a TapRef type descriptor.
 */
export function tapRefType(defaultValue: TapRef = createTapRef('google')): TapRefTypeDescriptor {
  return {
    kind: 'tapRef',
    displayName: 'TTS Tap Reference',

    validate(value: unknown): Result<TapRef> {
      if (typeof value !== 'string') {
        return err(`Expected string, got ${typeof value}`);
      }
      if (!/^[a-z][a-z0-9-]*$/.test(value)) {
        return err('Tap reference must be lowercase alphanumeric with hyphens, starting with a letter');
      }
      return ok(createTapRef(value));
    },

    getDefault(): TapRef {
      return defaultValue;
    },

    serialize(value: TapRef): unknown {
      return value as string;
    },

    deserialize(data: unknown): Result<TapRef> {
      return this.validate(data);
    },
  };
}

// =============================================================================
// Union Type for All Special Value Types
// =============================================================================

/**
 * Union of all special value type descriptors.
 */
export type AnySpecialValueTypeDescriptor =
  | VoiceChannelFollowingRuleTypeDescriptor
  | MemberFilterTypeDescriptor
  | MappingConfigTypeDescriptor
  | TapRefTypeDescriptor;
