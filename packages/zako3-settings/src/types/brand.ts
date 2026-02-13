/**
 * @fileoverview Branded type utilities for implementing the newtype pattern in TypeScript.
 *
 * Branded types provide compile-time type safety by creating distinct types from
 * primitive types. For example, `UserId` and `GuildId` are both strings at runtime,
 * but TypeScript treats them as incompatible types.
 *
 * @example
 * ```typescript
 * type UserId = Brand<string, 'UserId'>;
 * type GuildId = Brand<string, 'GuildId'>;
 *
 * const userId: UserId = 'abc' as UserId;
 * const guildId: GuildId = userId; // Error: Type 'UserId' is not assignable to type 'GuildId'
 * ```
 */

/**
 * Unique symbol used as the branding key.
 * This ensures the brand property doesn't conflict with any real properties.
 */
declare const BrandSymbol: unique symbol;

/**
 * A branded type that wraps a base type `T` with a unique brand `B`.
 *
 * @typeParam T - The underlying type (e.g., string, number)
 * @typeParam B - The brand identifier (typically a string literal type)
 */
export type Brand<T, B extends string> = T & { readonly [BrandSymbol]: B };

/**
 * Extracts the underlying type from a branded type.
 *
 * @typeParam T - The branded type
 */
export type Unbrand<T> = T extends Brand<infer U, string> ? U : T;

/**
 * Extracts the brand identifier from a branded type.
 *
 * @typeParam T - The branded type
 */
export type BrandOf<T> = T extends Brand<unknown, infer B> ? B : never;

/**
 * Creates a branded value from an unbranded value.
 * This is a zero-cost abstraction - no runtime overhead.
 *
 * @param value - The value to brand
 * @returns The branded value
 */
export function brand<T, B extends string>(value: T): Brand<T, B> {
  return value as Brand<T, B>;
}

/**
 * Removes the brand from a branded value.
 * This is a zero-cost abstraction - no runtime overhead.
 *
 * @param value - The branded value
 * @returns The unbranded value
 */
export function unbrand<T>(value: Brand<T, string>): T {
  return value as T;
}

/**
 * Type guard to check if a value can be branded.
 * Always returns true at runtime since branding is purely a compile-time concept.
 *
 * @param value - The value to check
 * @returns Always true (branding is compile-time only)
 */
export function isBrandable<T>(_value: T): _value is T {
  return true;
}

/**
 * Creates a factory function for creating branded values with optional validation.
 *
 * @typeParam T - The underlying type
 * @typeParam B - The brand identifier
 * @param validate - Optional validation function
 * @returns A factory function that creates branded values
 *
 * @example
 * ```typescript
 * const createUserId = makeBrandFactory<string, 'UserId'>(
 *   (value) => /^\d+$/.test(value) ? null : 'User ID must be numeric'
 * );
 *
 * const result = createUserId('12345'); // Ok<UserId>
 * const error = createUserId('abc');    // Err<'User ID must be numeric'>
 * ```
 */
export function makeBrandFactory<T, B extends string>(
  validate?: (value: T) => string | null
): (value: T) => BrandResult<T, B> {
  return (value: T): BrandResult<T, B> => {
    if (validate) {
      const error = validate(value);
      if (error !== null) {
        return { ok: false, error };
      }
    }
    return { ok: true, value: value as Brand<T, B> };
  };
}

/**
 * Result type for brand factory functions.
 */
export type BrandResult<T, B extends string> =
  | { readonly ok: true; readonly value: Brand<T, B> }
  | { readonly ok: false; readonly error: string };

/**
 * Creates a simple brand constructor without validation.
 * Use this when validation is not needed.
 *
 * @typeParam T - The underlying type
 * @typeParam B - The brand identifier
 * @returns A function that brands values
 *
 * @example
 * ```typescript
 * const UserId = makeBrandConstructor<string, 'UserId'>();
 * const id = UserId('12345'); // Type: Brand<string, 'UserId'>
 * ```
 */
export function makeBrandConstructor<T, B extends string>(): (value: T) => Brand<T, B> {
  return (value: T): Brand<T, B> => value as Brand<T, B>;
}
