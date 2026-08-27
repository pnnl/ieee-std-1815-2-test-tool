// Routes ValidationError entries to the panel that owns them.
//
// Routing reads only `point`, never `message`: `message` is free-form prose
// the backend can reword at any time, so it is not a safe discriminator.
//
// Point-tab prefixes (AO/AI/BO/BI) are anchored at both ends (the whole
// string must be `<prefix><digits>`), so a composite point like
// "AI12 and AI13" cannot false-match analog_inputs.
//
// Curve/schedule errors carry a formatted display-name string in `point`
// instead of a bare point id, so they route by an anchored prefix on that
// string:
//   - `<Curve of type '...'>: ...`                    -> curves
//   - `Schedule <N>` or `Schedule <N>.<field>[...]`    -> scheduling
//
// FRAGILITY NOTE: both display-name prefixes are prose from a backend
// `display_name()` function, not a structured discriminator. A backend
// reword silently reverts affected errors to `general`, with nothing failing.
//
// Everything else, including the empty string and structural workbook
// faults, lands in `general`. Nothing is dropped: bucket lengths always
// sum to the input length.

import type { ValidationError } from '@/api/generated'
import type { PointSection } from './canonical'

export type ValidationBucketId =
  PointSection | 'curves' | 'scheduling' | 'general'

export type ValidationBuckets = Record<ValidationBucketId, ValidationError[]>

const POINT_PREFIX_TO_TAB: Record<string, PointSection> = {
  AO: 'analog_outputs',
  AI: 'analog_inputs',
  BO: 'binary_outputs',
  BI: 'binary_inputs',
}

// Full-string match only: prefix, one or more digits, nothing else.
const PREFIXED_POINT = /^(AO|AI|BO|BI)(\d+)$/

// Anchored at the start only: both display names are followed by a
// variable suffix, so a full-string match would never fire. See the
// module doc comment above for the fragility this carries.
const CURVE_DISPLAY_NAME_PREFIX = "<Curve of type '"
const SCHEDULE_DISPLAY_NAME_PREFIX = 'Schedule '

function emptyBuckets(): ValidationBuckets {
  return {
    analog_outputs: [],
    analog_inputs: [],
    binary_outputs: [],
    binary_inputs: [],
    curves: [],
    scheduling: [],
    general: [],
  }
}

function classify(point: string): ValidationBucketId {
  const match = PREFIXED_POINT.exec(point)
  if (match) return POINT_PREFIX_TO_TAB[match[1]]
  if (point.startsWith(CURVE_DISPLAY_NAME_PREFIX)) return 'curves'
  if (point.startsWith(SCHEDULE_DISPLAY_NAME_PREFIX)) return 'scheduling'
  return 'general'
}

export function bucketValidationErrors(
  errors: readonly ValidationError[],
): ValidationBuckets {
  const buckets = emptyBuckets()

  for (const error of errors) {
    buckets[classify(error.point)].push(error)
  }

  return buckets
}

// Uses the same anchored pattern `bucketValidationErrors` routes with.
// Callers pass in only errors already in a tab's bucket, so a null return
// signals a caller skipped bucketing first, not a real "no index" case.
export function extractPointIndex(point: string): number | null {
  const match = PREFIXED_POINT.exec(point)
  return match ? Number(match[2]) : null
}
