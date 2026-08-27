// Response-shape interpretation for the JSON `/validate` import path.
//
// `ValidationErrorsResponse` is `{ errors: { errors: ValidationError[] } }`.
// Shape is answered from the parsed body, not the HTTP status code.

import type { ValidationError, ValidationErrorsResponse } from '@/api/generated'

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

export function isValidationErrorsResponse(
  value: unknown,
): value is ValidationErrorsResponse {
  if (!isRecord(value)) return false
  const errors = value.errors
  if (!isRecord(errors)) return false
  return Array.isArray(errors.errors)
}

// An empty `errors` array is "no errors found"; a missing/null/non-array
// `errors` is not an answer at all and must not read the same. Once past
// that envelope check, a malformed individual element is degraded (a
// placeholder message), not discarded: dropping the whole list over one
// bad element would be worse for the user than showing every entry.
function normalizeValidationError(raw: unknown): ValidationError {
  const record = isRecord(raw) ? raw : {}
  const point = typeof record.point === 'string' ? record.point : ''
  const message =
    typeof record.message === 'string' && record.message.length > 0
      ? record.message
      : '(malformed error entry: no message provided)'
  return { point, message }
}

function normalizeValidationErrors(raw: unknown[]): ValidationError[] {
  return raw.map(normalizeValidationError)
}

// `clean`: no validation errors. `invalid`: failed with a structured
// per-point list. `unconfirmed`: the request couldn't answer the validity
// question at all; this is NOT `clean`, an empty error set must never be
// synthesized from a request that told us nothing.
export type ValidateOutcome =
  | { kind: 'clean' }
  | { kind: 'invalid'; errors: ValidationError[] }
  | { kind: 'unconfirmed'; message: string }

// Interprets the SDK's `{ error }` result from the JSON-body `/validate`
// call (used for both the JSON import path and the explicit Validate
// button). `error` is `undefined` on a 200.
export function interpretValidateError(error: unknown): ValidateOutcome {
  if (error == null) {
    return { kind: 'clean' }
  }

  if (isValidationErrorsResponse(error)) {
    return {
      kind: 'invalid',
      errors: normalizeValidationErrors(error.errors.errors),
    }
  }

  const message =
    typeof error === 'string' ? error : 'Validation request failed.'
  return { kind: 'unconfirmed', message }
}
