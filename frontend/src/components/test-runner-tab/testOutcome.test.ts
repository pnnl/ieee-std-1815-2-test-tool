import { describe, it, expect } from 'vitest'
import { deriveOutcome } from './testOutcome'

// ---------------------------------------------------------------------------
// deriveOutcome: full truth table
//
// passed can be true, false, null, or undefined.
// shouldPass can be true or false.
// ---------------------------------------------------------------------------

describe('deriveOutcome: truth table', () => {
  // passed=undefined (no result yet): always pending, regardless of shouldPass.
  it('returns pending when passed=undefined and shouldPass=true', () => {
    expect(deriveOutcome(undefined, true)).toBe('pending')
  })

  it('returns pending when passed=undefined and shouldPass=false', () => {
    expect(deriveOutcome(undefined, false)).toBe('pending')
  })

  // passed=null (tri-state: result arrived but indeterminate): always pending.
  it('returns pending when passed=null and shouldPass=true', () => {
    expect(deriveOutcome(null, true)).toBe('pending')
  })

  it('returns pending when passed=null and shouldPass=false', () => {
    expect(deriveOutcome(null, false)).toBe('pending')
  })

  // passed=true: outcome depends on shouldPass.
  it('returns passed when passed=true and shouldPass=true', () => {
    expect(deriveOutcome(true, true)).toBe('passed')
  })

  it('returns failed when passed=true and shouldPass=false (expected-to-fail but passed)', () => {
    expect(deriveOutcome(true, false)).toBe('failed')
  })

  // passed=false: outcome depends on shouldPass.
  it('returns failed when passed=false and shouldPass=true', () => {
    expect(deriveOutcome(false, true)).toBe('failed')
  })

  it('returns passed when passed=false and shouldPass=false (expected-to-fail, and did fail)', () => {
    expect(deriveOutcome(false, false)).toBe('passed')
  })
})
