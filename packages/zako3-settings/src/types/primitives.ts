/**
 * @fileoverview Primitive value types for settings.
 *
 * This module provides type-safe wrappers for primitive setting values,
 * including validation support for ranges and patterns.
 */

import { type Result, ok, err } from './result';

// =============================================================================
// Value Type Descriptors
// =============================================================================

/**
 * Base interface for all value type descriptors.
 * Value type descriptors define the type and validation rules for setting values.
 */
export interface ValueTypeDescriptor<T> {
  /** Unique identifier for this value type */
  readonly kind: string;

  /** Human-readable name for UI display */
  readonly displayName: string;

  /** Validates a value against this type's rules */
  validate(value: unknown): Result<T>;

  /** Returns the default value for this type */
  getDefault(): T;

  /** Serializes a value to JSON-compatible format */
  serialize(value: T): unknown;

  /** Deserializes a value from JSON-compatible format */
  deserialize(data: unknown): Result<T>;
}

// =============================================================================
// Boolean Type
// =============================================================================

/**
 * Descriptor for boolean settings.
 */
export interface BooleanTypeDescriptor extends ValueTypeDescriptor<boolean> {
  readonly kind: 'boolean';
}

/**
 * Creates a boolean type descriptor.
 *
 * @param defaultValue - The default value (defaults to false)
 * @returns A boolean type descriptor
 */
export function booleanType(defaultValue: boolean = false): BooleanTypeDescriptor {
  return {
    kind: 'boolean',
    displayName: 'Boolean',

    validate(value: unknown): Result<boolean> {
      if (typeof value !== 'boolean') {
        return err(`Expected boolean, got ${typeof value}`);
      }
      return ok(value);
    },

    getDefault(): boolean {
      return defaultValue;
    },

    serialize(value: boolean): unknown {
      return value;
    },

    deserialize(data: unknown): Result<boolean> {
      return this.validate(data);
    },
  };
}

// =============================================================================
// Integer Type
// =============================================================================

/**
 * Range constraint for integer values.
 */
export interface IntegerRange {
  /** Minimum value (inclusive) */
  readonly min?: number;
  /** Maximum value (inclusive) */
  readonly max?: number;
}

/**
 * Descriptor for integer settings with optional range validation.
 */
export interface IntegerTypeDescriptor extends ValueTypeDescriptor<number> {
  readonly kind: 'integer';
  readonly range: IntegerRange | null;
}

/**
 * Creates an integer type descriptor.
 *
 * @param defaultValue - The default value (defaults to 0)
 * @param range - Optional range constraint
 * @returns An integer type descriptor
 */
export function integerType(
  defaultValue: number = 0,
  range?: IntegerRange
): IntegerTypeDescriptor {
  const rangeOrNull = range ?? null;

  function validateRange(value: number): string | null {
    if (rangeOrNull) {
      if (rangeOrNull.min !== undefined && value < rangeOrNull.min) {
        return `Value ${value} is below minimum ${rangeOrNull.min}`;
      }
      if (rangeOrNull.max !== undefined && value > rangeOrNull.max) {
        return `Value ${value} is above maximum ${rangeOrNull.max}`;
      }
    }
    return null;
  }

  return {
    kind: 'integer',
    displayName: rangeOrNull
      ? `Integer (${rangeOrNull.min ?? '-∞'} to ${rangeOrNull.max ?? '∞'})`
      : 'Integer',
    range: rangeOrNull,

    validate(value: unknown): Result<number> {
      if (typeof value !== 'number') {
        return err(`Expected number, got ${typeof value}`);
      }
      if (!Number.isInteger(value)) {
        return err(`Expected integer, got float ${value}`);
      }
      const rangeError = validateRange(value);
      if (rangeError) {
        return err(rangeError);
      }
      return ok(value);
    },

    getDefault(): number {
      return defaultValue;
    },

    serialize(value: number): unknown {
      return value;
    },

    deserialize(data: unknown): Result<number> {
      return this.validate(data);
    },
  };
}

// =============================================================================
// String Type
// =============================================================================

/**
 * Pattern constraint for string values.
 */
export interface StringPattern {
  /** Regular expression pattern */
  readonly pattern: RegExp;
  /** Human-readable description of valid format */
  readonly description: string;
}

/**
 * Descriptor for string settings with optional pattern validation.
 */
export interface StringTypeDescriptor extends ValueTypeDescriptor<string> {
  readonly kind: 'string';
  readonly pattern: StringPattern | null;
  readonly maxLength: number | null;
}

/**
 * Creates a string type descriptor.
 *
 * @param defaultValue - The default value (defaults to '')
 * @param options - Optional constraints
 * @returns A string type descriptor
 */
export function stringType(
  defaultValue: string = '',
  options?: {
    pattern?: StringPattern;
    maxLength?: number;
  }
): StringTypeDescriptor {
  const pattern = options?.pattern ?? null;
  const maxLength = options?.maxLength ?? null;

  return {
    kind: 'string',
    displayName: pattern ? `String (${pattern.description})` : 'String',
    pattern,
    maxLength,

    validate(value: unknown): Result<string> {
      if (typeof value !== 'string') {
        return err(`Expected string, got ${typeof value}`);
      }
      if (maxLength !== null && value.length > maxLength) {
        return err(`String length ${value.length} exceeds maximum ${maxLength}`);
      }
      if (pattern && !pattern.pattern.test(value)) {
        return err(`String does not match required format: ${pattern.description}`);
      }
      return ok(value);
    },

    getDefault(): string {
      return defaultValue;
    },

    serialize(value: string): unknown {
      return value;
    },

    deserialize(data: unknown): Result<string> {
      return this.validate(data);
    },
  };
}

// =============================================================================
// SomeOrDefault Type
// =============================================================================

/**
 * Represents a value that can be either explicitly set or use a default.
 * Similar to Option<T> but with semantic meaning: "user chose default" vs "user chose a specific value".
 */
export type SomeOrDefault<T> =
  | { readonly kind: 'default' }
  | { readonly kind: 'some'; readonly value: T };

/**
 * Creates a "default" variant of SomeOrDefault.
 */
export function useDefault<T>(): SomeOrDefault<T> {
  return { kind: 'default' };
}

/**
 * Creates a "some" variant of SomeOrDefault.
 */
export function useSome<T>(value: T): SomeOrDefault<T> {
  return { kind: 'some', value };
}

/**
 * Checks if a SomeOrDefault is the "default" variant.
 */
export function isDefault<T>(value: SomeOrDefault<T>): value is { kind: 'default' } {
  return value.kind === 'default';
}

/**
 * Checks if a SomeOrDefault is the "some" variant.
 */
export function isSome<T>(value: SomeOrDefault<T>): value is { kind: 'some'; value: T } {
  return value.kind === 'some';
}

/**
 * Unwraps a SomeOrDefault, returning the value or a fallback.
 */
export function unwrapOrDefault<T>(value: SomeOrDefault<T>, fallback: T): T {
  return value.kind === 'some' ? value.value : fallback;
}

/**
 * Descriptor for SomeOrDefault settings.
 */
export interface SomeOrDefaultTypeDescriptor<T> extends ValueTypeDescriptor<SomeOrDefault<T>> {
  readonly kind: 'someOrDefault';
  readonly innerType: ValueTypeDescriptor<T>;
}

/**
 * Creates a SomeOrDefault type descriptor.
 *
 * @param innerType - The type descriptor for the inner value
 * @returns A SomeOrDefault type descriptor
 */
export function someOrDefaultType<T>(
  innerType: ValueTypeDescriptor<T>
): SomeOrDefaultTypeDescriptor<T> {
  return {
    kind: 'someOrDefault',
    displayName: `Optional ${innerType.displayName}`,
    innerType,

    validate(value: unknown): Result<SomeOrDefault<T>> {
      if (typeof value !== 'object' || value === null) {
        return err(`Expected object with kind property, got ${typeof value}`);
      }

      const obj = value as Record<string, unknown>;

      if (obj.kind === 'default') {
        return ok(useDefault());
      }

      if (obj.kind === 'some') {
        const innerResult = innerType.validate(obj.value);
        if (!innerResult.ok) {
          return err(`Invalid inner value: ${innerResult.error}`);
        }
        return ok(useSome(innerResult.value));
      }

      return err(`Expected kind to be 'default' or 'some', got '${obj.kind}'`);
    },

    getDefault(): SomeOrDefault<T> {
      return useDefault();
    },

    serialize(value: SomeOrDefault<T>): unknown {
      if (value.kind === 'default') {
        return { kind: 'default' };
      }
      return { kind: 'some', value: innerType.serialize(value.value) };
    },

    deserialize(data: unknown): Result<SomeOrDefault<T>> {
      return this.validate(data);
    },
  };
}

// =============================================================================
// List Type
// =============================================================================

/**
 * Descriptor for list/array settings.
 */
export interface ListTypeDescriptor<T> extends ValueTypeDescriptor<readonly T[]> {
  readonly kind: 'list';
  readonly itemType: ValueTypeDescriptor<T>;
  readonly minLength: number | null;
  readonly maxLength: number | null;
}

/**
 * Creates a list type descriptor.
 *
 * @param itemType - The type descriptor for list items
 * @param options - Optional constraints
 * @returns A list type descriptor
 */
export function listType<T>(
  itemType: ValueTypeDescriptor<T>,
  options?: {
    defaultValue?: readonly T[];
    minLength?: number;
    maxLength?: number;
  }
): ListTypeDescriptor<T> {
  const defaultValue = options?.defaultValue ?? [];
  const minLength = options?.minLength ?? null;
  const maxLength = options?.maxLength ?? null;

  return {
    kind: 'list',
    displayName: `List of ${itemType.displayName}`,
    itemType,
    minLength,
    maxLength,

    validate(value: unknown): Result<readonly T[]> {
      if (!Array.isArray(value)) {
        return err(`Expected array, got ${typeof value}`);
      }

      if (minLength !== null && value.length < minLength) {
        return err(`Array length ${value.length} is below minimum ${minLength}`);
      }

      if (maxLength !== null && value.length > maxLength) {
        return err(`Array length ${value.length} exceeds maximum ${maxLength}`);
      }

      const validated: T[] = [];
      for (let i = 0; i < value.length; i++) {
        const itemResult = itemType.validate(value[i]);
        if (!itemResult.ok) {
          return err(`Invalid item at index ${i}: ${itemResult.error}`);
        }
        validated.push(itemResult.value);
      }

      return ok(validated);
    },

    getDefault(): readonly T[] {
      return defaultValue;
    },

    serialize(value: readonly T[]): unknown {
      return value.map((item) => itemType.serialize(item));
    },

    deserialize(data: unknown): Result<readonly T[]> {
      return this.validate(data);
    },
  };
}

// =============================================================================
// Union Type for All Value Types
// =============================================================================

/**
 * Union of all value type descriptors.
 */
export type AnyValueTypeDescriptor =
  | BooleanTypeDescriptor
  | IntegerTypeDescriptor
  | StringTypeDescriptor
  | SomeOrDefaultTypeDescriptor<unknown>
  | ListTypeDescriptor<unknown>;
