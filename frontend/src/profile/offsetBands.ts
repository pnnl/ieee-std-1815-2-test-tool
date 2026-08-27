// Single band-lookup used everywhere a raw point index needs to resolve to
// one of the `SECTION_OFFSETS` bands: grouping a tab's points for
// `OffsetSection` rendering, and counting validation errors per group for
// the group-header badges. Both consumers go through `resolveOffsetName`,
// so the band boundaries can never drift between the two.
//
// This module is also the single home for error-to-point resolution:
// `groupErrorsByPointIndex` below is the one place `extractPointIndex` gets
// used to build a per-point lookup, reused by both the group badge
// (`groupErrorCountsByOffset`) and the per-row highlight. Do not build a
// second lookup elsewhere.

import { SECTION_OFFSETS, type FlatPoint } from './canonical'
import { extractPointIndex } from './validationBucketing'
import type { ValidationError } from '@/api/generated'

// Band boundaries, sorted ascending once at module load.
const OFFSET_ENTRIES = Object.entries(SECTION_OFFSETS).sort(
  (a, b) => a[1] - b[1],
)

function resolveOffsetName(index: number): string {
  let assignedGroup = OFFSET_ENTRIES[0][0]
  for (let i = 0; i < OFFSET_ENTRIES.length; i++) {
    const [name, value] = OFFSET_ENTRIES[i]
    const nextValue =
      i < OFFSET_ENTRIES.length - 1 ? OFFSET_ENTRIES[i + 1][1] : Infinity
    if (index >= value && index < nextValue) {
      assignedGroup = name
      break
    }
  }
  return assignedGroup
}

export function groupPointsByOffset(
  points: FlatPoint[],
): Record<string, FlatPoint[]> {
  const grouped: Record<string, FlatPoint[]> = {}
  for (const [name] of OFFSET_ENTRIES) {
    grouped[name] = []
  }
  for (const point of points) {
    grouped[resolveOffsetName(point.point_index)].push(point)
  }
  return grouped
}

// Counts validation errors per offset band, for the group-header badges.
// A null `extractPointIndex` here means a caller skipped bucketing first;
// such an error is deliberately left uncounted rather than force-assigned
// to an arbitrary band.
export function groupErrorCountsByOffset(
  errors: readonly ValidationError[],
): Record<string, number> {
  const counts: Record<string, number> = {}
  for (const [name] of OFFSET_ENTRIES) {
    counts[name] = 0
  }
  for (const error of errors) {
    const idx = extractPointIndex(error.point)
    if (idx === null) continue
    counts[resolveOffsetName(idx)] += 1
  }
  return counts
}

// Groups validation errors by the point index they resolve to, for the
// per-row highlight. Deliberately NOT deduplicated, so callers can render
// every message in the row's tooltip. `map.size` is ERRORED POINTS, not
// ERRORS, so it won't reconcile with `groupErrorCountsByOffset`'s count.
export function groupErrorsByPointIndex(
  errors: readonly ValidationError[],
): Map<number, ValidationError[]> {
  const map = new Map<number, ValidationError[]>()
  for (const error of errors) {
    const idx = extractPointIndex(error.point)
    if (idx === null) continue
    const existing = map.get(idx)
    if (existing) {
      existing.push(error)
    } else {
      map.set(idx, [error])
    }
  }
  return map
}
