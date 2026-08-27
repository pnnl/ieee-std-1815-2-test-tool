// Tests for the JSON `/validate` response-shape contract: the branch is on
// what the body carries, not on the HTTP status code.

import { describe, it, expect } from 'vitest'
import type { PicsProfile } from '@/api/generated'
import {
  interpretValidateError,
  isValidationErrorsResponse,
} from './importResponse'

const PROFILE: PicsProfile = {
  AI: {} as PicsProfile['AI'],
  AO: {} as PicsProfile['AO'],
  BI: {} as PicsProfile['BI'],
  BO: {} as PicsProfile['BO'],
  CTR: [],
  Key: {} as PicsProfile['Key'],
}

describe('isValidationErrorsResponse', () => {
  it('accepts the { errors: { errors: [] } } shape: an empty array is a legitimate, well-formed answer', () => {
    expect(isValidationErrorsResponse({ errors: { errors: [] } })).toBe(true)
  })

  it('rejects a bare PicsProfile', () => {
    expect(isValidationErrorsResponse(PROFILE)).toBe(false)
  })

  it('rejects a body with no "errors" key at all', () => {
    expect(isValidationErrorsResponse({})).toBe(false)
  })

  it('rejects "errors" present but the inner "errors" array key missing', () => {
    expect(isValidationErrorsResponse({ errors: {} })).toBe(false)
  })

  it('rejects errors.errors: null', () => {
    expect(isValidationErrorsResponse({ errors: { errors: null } })).toBe(false)
  })

  it('rejects errors.errors present but not an array', () => {
    expect(
      isValidationErrorsResponse({ errors: { errors: 'not-an-array' } }),
    ).toBe(false)
    expect(isValidationErrorsResponse({ errors: { errors: { 0: 'x' } } })).toBe(
      false,
    )
  })
})

describe('interpretValidateError', () => {
  it('no error: clean', () => {
    expect(interpretValidateError(undefined)).toEqual({ kind: 'clean' })
    expect(interpretValidateError(null)).toEqual({ kind: 'clean' })
  })

  it('a ValidationErrorsResponse: invalid, carrying the errors', () => {
    const error = { errors: { errors: [{ point: 'AI3', message: 'bad' }] } }
    expect(interpretValidateError(error)).toEqual({
      kind: 'invalid',
      errors: [{ point: 'AI3', message: 'bad' }],
    })
  })

  it('a transport failure (non-shaped error object): unconfirmed with a generic message', () => {
    expect(interpretValidateError(new TypeError('network down'))).toEqual({
      kind: 'unconfirmed',
      message: 'Validation request failed.',
    })
  })

  it('a raw string error: unconfirmed, carrying the string as the message', () => {
    expect(interpretValidateError('server unavailable')).toEqual({
      kind: 'unconfirmed',
      message: 'server unavailable',
    })
  })

  it('a well-formed but empty errors array: invalid, not unconfirmed, carrying zero errors', () => {
    expect(interpretValidateError({ errors: { errors: [] } })).toEqual({
      kind: 'invalid',
      errors: [],
    })
  })

  it('the outer "errors" key missing: unconfirmed, not clean', () => {
    expect(interpretValidateError({})).toEqual({
      kind: 'unconfirmed',
      message: 'Validation request failed.',
    })
  })

  it('the inner "errors" array key missing: unconfirmed, not clean', () => {
    expect(interpretValidateError({ errors: {} })).toEqual({
      kind: 'unconfirmed',
      message: 'Validation request failed.',
    })
  })

  it('errors.errors: null: unconfirmed, not clean', () => {
    expect(interpretValidateError({ errors: { errors: null } })).toEqual({
      kind: 'unconfirmed',
      message: 'Validation request failed.',
    })
  })

  it('errors.errors present but not an array: unconfirmed, not clean', () => {
    expect(
      interpretValidateError({ errors: { errors: 'not-an-array' } }),
    ).toEqual({
      kind: 'unconfirmed',
      message: 'Validation request failed.',
    })
  })

  it('a malformed element is degraded, not dropped', () => {
    const result = interpretValidateError({
      errors: { errors: [{ point: 'AI3' }, { message: 'no point' }] },
    })
    expect(result).toEqual({
      kind: 'invalid',
      errors: [
        {
          point: 'AI3',
          message: '(malformed error entry: no message provided)',
        },
        { point: '', message: 'no point' },
      ],
    })
  })
})
