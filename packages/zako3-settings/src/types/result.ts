/**
 * @fileoverview Result type for error handling, inspired by Rust's Result<T, E>.
 *
 * This module provides a type-safe way to handle operations that can fail,
 * without using exceptions. It encourages explicit error handling and makes
 * the possibility of failure visible in function signatures.
 */

// =============================================================================
// Core Result Type
// =============================================================================

/**
 * Represents a successful result containing a value.
 */
export interface Ok<T> {
  readonly ok: true;
  readonly value: T;
}

/**
 * Represents a failed result containing an error.
 */
export interface Err<E> {
  readonly ok: false;
  readonly error: E;
}

/**
 * A type that represents either success (Ok) or failure (Err).
 *
 * @typeParam T - The success value type
 * @typeParam E - The error type (defaults to string)
 */
export type Result<T, E = string> = Ok<T> | Err<E>;

// =============================================================================
// Constructors
// =============================================================================

/**
 * Creates a successful result.
 *
 * @param value - The success value
 * @returns An Ok result containing the value
 */
export function ok<T>(value: T): Ok<T> {
  return { ok: true, value };
}

/**
 * Creates a failed result.
 *
 * @param error - The error value
 * @returns An Err result containing the error
 */
export function err<E>(error: E): Err<E> {
  return { ok: false, error };
}

// =============================================================================
// Type Guards
// =============================================================================

/**
 * Type guard to check if a result is Ok.
 *
 * @param result - The result to check
 * @returns True if the result is Ok
 */
export function isOk<T, E>(result: Result<T, E>): result is Ok<T> {
  return result.ok;
}

/**
 * Type guard to check if a result is Err.
 *
 * @param result - The result to check
 * @returns True if the result is Err
 */
export function isErr<T, E>(result: Result<T, E>): result is Err<E> {
  return !result.ok;
}

// =============================================================================
// Unwrapping
// =============================================================================

/**
 * Unwraps a result, returning the value if Ok, or throwing if Err.
 *
 * @param result - The result to unwrap
 * @param message - Optional custom error message
 * @returns The unwrapped value
 * @throws Error if the result is Err
 */
export function unwrap<T, E>(result: Result<T, E>, message?: string): T {
  if (result.ok) {
    return result.value;
  }
  throw new Error(message ?? `Unwrap called on Err: ${result.error}`);
}

/**
 * Unwraps a result, returning the value if Ok, or a default value if Err.
 *
 * @param result - The result to unwrap
 * @param defaultValue - The default value to return if Err
 * @returns The unwrapped value or the default
 */
export function unwrapOr<T, E>(result: Result<T, E>, defaultValue: T): T {
  return result.ok ? result.value : defaultValue;
}

/**
 * Unwraps a result, returning the value if Ok, or calling a function to get a default if Err.
 *
 * @param result - The result to unwrap
 * @param fn - Function to call if Err, receives the error
 * @returns The unwrapped value or the function result
 */
export function unwrapOrElse<T, E>(result: Result<T, E>, fn: (error: E) => T): T {
  return result.ok ? result.value : fn(result.error);
}

/**
 * Unwraps the error from a result, throwing if Ok.
 *
 * @param result - The result to unwrap
 * @param message - Optional custom error message
 * @returns The unwrapped error
 * @throws Error if the result is Ok
 */
export function unwrapErr<T, E>(result: Result<T, E>, message?: string): E {
  if (!result.ok) {
    return result.error;
  }
  throw new Error(message ?? `UnwrapErr called on Ok: ${result.value}`);
}

// =============================================================================
// Transformations
// =============================================================================

/**
 * Maps a Result<T, E> to Result<U, E> by applying a function to the Ok value.
 *
 * @param result - The result to map
 * @param fn - The function to apply to the Ok value
 * @returns A new result with the mapped value
 */
export function map<T, U, E>(result: Result<T, E>, fn: (value: T) => U): Result<U, E> {
  return result.ok ? ok(fn(result.value)) : result;
}

/**
 * Maps a Result<T, E> to Result<T, F> by applying a function to the Err value.
 *
 * @param result - The result to map
 * @param fn - The function to apply to the Err value
 * @returns A new result with the mapped error
 */
export function mapErr<T, E, F>(result: Result<T, E>, fn: (error: E) => F): Result<T, F> {
  return result.ok ? result : err(fn(result.error));
}

/**
 * Chains results by applying a function that returns a Result to an Ok value.
 *
 * @param result - The result to chain
 * @param fn - The function to apply to the Ok value
 * @returns The result from the function, or the original Err
 */
export function flatMap<T, U, E>(
  result: Result<T, E>,
  fn: (value: T) => Result<U, E>
): Result<U, E> {
  return result.ok ? fn(result.value) : result;
}

/**
 * Alias for flatMap.
 */
export const andThen = flatMap;

// =============================================================================
// Combining Results
// =============================================================================

/**
 * Combines an array of Results into a Result of an array.
 * Returns the first Err encountered, or Ok with all values.
 *
 * @param results - The array of results to combine
 * @returns Ok with all values, or the first Err
 */
export function all<T, E>(results: readonly Result<T, E>[]): Result<T[], E> {
  const values: T[] = [];
  for (const result of results) {
    if (!result.ok) {
      return result;
    }
    values.push(result.value);
  }
  return ok(values);
}

/**
 * Returns the first Ok result, or the last Err if all are Err.
 *
 * @param results - The array of results to check
 * @returns The first Ok, or the last Err
 */
export function any<T, E>(results: readonly Result<T, E>[]): Result<T, E> {
  let lastErr: Err<E> | undefined;
  for (const result of results) {
    if (result.ok) {
      return result;
    }
    lastErr = result;
  }
  // If we get here, all results were Err
  // TypeScript knows lastErr must be defined if results is non-empty
  return lastErr!;
}

// =============================================================================
// Async Support
// =============================================================================

/**
 * Wraps a Promise that might reject into a Promise<Result>.
 *
 * @param promise - The promise to wrap
 * @param mapError - Optional function to map the caught error
 * @returns A Promise<Result> that never rejects
 */
export async function fromPromise<T, E = unknown>(
  promise: Promise<T>,
  mapError?: (error: unknown) => E
): Promise<Result<T, E>> {
  try {
    const value = await promise;
    return ok(value);
  } catch (e) {
    return err(mapError ? mapError(e) : (e as E));
  }
}

/**
 * Converts a Result to a Promise that rejects on Err.
 *
 * @param result - The result to convert
 * @returns A Promise that resolves with the value or rejects with the error
 */
export function toPromise<T, E>(result: Result<T, E>): Promise<T> {
  return result.ok ? Promise.resolve(result.value) : Promise.reject(result.error);
}

// =============================================================================
// Pattern Matching
// =============================================================================

/**
 * Pattern matches on a Result, calling the appropriate handler.
 *
 * @param result - The result to match
 * @param handlers - Object with onOk and onErr handlers
 * @returns The result of the called handler
 */
export function match<T, E, U>(
  result: Result<T, E>,
  handlers: {
    onOk: (value: T) => U;
    onErr: (error: E) => U;
  }
): U {
  return result.ok ? handlers.onOk(result.value) : handlers.onErr(result.error);
}
