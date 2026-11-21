/**
 * Small helpers shared by the other modules. Only `JsonObject` is part of the public API.
 *
 * @module
 */

/** A JSON object as returned by the API. */
export type JsonObject = Record<string, unknown>;

/** `true` for plain objects (not arrays, not `null`). */
export function isObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Truthiness the way the API (and the Python reference client) means it:
 * empty strings, empty arrays, empty objects, `0`, `false`, `null` and `undefined` are falsy.
 */
export function isTruthy(value: unknown): boolean {
  if (Array.isArray(value)) return value.length > 0;
  if (isObject(value)) return Object.keys(value).length > 0;
  return Boolean(value);
}

/** Encode one path segment of a URL. */
export function segment(value: string): string {
  return encodeURIComponent(value);
}
