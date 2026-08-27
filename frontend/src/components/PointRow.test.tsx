// Tests for the row-level validation error highlight. Same "red means
// something" principle as the tab and group badges: an unerrored row
// renders unstyled, and a point with more than one error (AI2) gets
// exactly ONE highlighted row, never a doubled style.

import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Theme } from '@radix-ui/themes'
import { Table, TableBody } from '@/components/ui/table'
import PointRow from './PointRow'
import type { FlatAiPoint } from '@/profile/canonical'
import type { ValidationError } from '@/api/generated'

function renderRow(point: FlatAiPoint, errors?: ValidationError[]) {
  return render(
    <Theme>
      <Table>
        <TableBody>
          <PointRow
            point={point}
            isBinary={false}
            showValueColumn={true}
            onUpdate={() => {}}
            errors={errors}
          />
        </TableBody>
      </Table>
    </Theme>,
  )
}

function makeAiPoint(
  pointIndex: number,
  overrides: Partial<FlatAiPoint> = {},
): FlatAiPoint {
  return {
    kind: 'ai',
    locator: { kind: 'ai', group: 'points', index: pointIndex },
    point_index: pointIndex,
    name: `Point ${pointIndex}`,
    purpose: '',
    units: '',
    value: 0,
    minimum: 0,
    maximum: 100,
    multiplier: 1,
    offset: 0,
    mandatory_1547: false,
    mandatory_1815: false,
    iec_61850_uid: '',
    event_class: 'None',
    ...overrides,
  }
}

function err(point: string, message: string): ValidationError {
  return { point, message }
}

describe('PointRow error highlight', () => {
  it('renders unstyled when errors is omitted', () => {
    renderRow(makeAiPoint(0))
    const row = screen.getByRole('row')
    expect(row.className).not.toMatch(/destructive/)
    expect(row).not.toHaveAttribute('title')
    expect(row).not.toHaveAttribute('aria-describedby')
  })

  it('renders unstyled when errors is an empty array', () => {
    renderRow(makeAiPoint(0), [])
    const row = screen.getByRole('row')
    expect(row.className).not.toMatch(/destructive/)
  })

  it('highlights the row for a single error and carries the message as a title and a screen-reader description', () => {
    renderRow(makeAiPoint(0), [
      err('AI0', 'Value must be between 1 and 100, but is 999'),
    ])
    const row = screen.getByRole('row')
    expect(row.className).toMatch(/bg-destructive\/10/)
    expect(row.className).toMatch(/border-l-destructive/)
    expect(row).toHaveAttribute(
      'title',
      'Value must be between 1 and 100, but is 999',
    )
    const describedBy = row.getAttribute('aria-describedby')
    expect(describedBy).toBeTruthy()
    expect(document.getElementById(describedBy!)).toHaveTextContent(
      'Value must be between 1 and 100, but is 999',
    )
  })

  // AI2 carries two errors: one highlighted row, one title joining both
  // messages.
  it('highlights a two-error point exactly once, with both messages in the title', () => {
    renderRow(makeAiPoint(2), [
      err('AI2', 'Minimum value cannot be greater than maximum value'),
      err('AI2', 'Value must be between 10 and 0, but is 0'),
    ])
    const rows = screen.getAllByRole('row')
    expect(rows).toHaveLength(1)
    const row = rows[0]
    expect(row.className).toMatch(/bg-destructive\/10/)
    expect(row).toHaveAttribute(
      'title',
      'Minimum value cannot be greater than maximum value; Value must be between 10 and 0, but is 0',
    )
  })

  it('the error highlight takes precedence over the mandatory_1815 styling', () => {
    renderRow(makeAiPoint(0, { mandatory_1815: true }), [err('AI0', 'boom')])
    const row = screen.getByRole('row')
    expect(row.className).toMatch(/destructive/)
    expect(row.className).not.toMatch(/border-l-primary/)
  })

  it('a mandatory point with no errors keeps its existing primary styling (regression guard)', () => {
    renderRow(makeAiPoint(0, { mandatory_1815: true }))
    const row = screen.getByRole('row')
    expect(row.className).toMatch(/border-l-primary/)
    expect(row.className).not.toMatch(/destructive/)
  })

  // An errored row's hover state must deepen the red instead of falling
  // through to TableRow's standard hover:bg-muted/50, which would erase
  // the error color the instant the user points at the row.
  it('an errored row carries a deeper destructive hover than the standard row hover', () => {
    renderRow(makeAiPoint(0), [err('AI0', 'boom')])
    const row = screen.getByRole('row')
    expect(row.className).toMatch(/hover:bg-destructive\/25/)
    expect(row.className).toMatch(/dark:hover:bg-destructive\/35/)
    // twMerge must have dropped TableRow's own hover:bg-muted/50, not
    // merely appended ours alongside it.
    expect(row.className).not.toMatch(/hover:bg-muted/)
  })

  it('a clean row keeps the standard hover, with no destructive hover class', () => {
    renderRow(makeAiPoint(0))
    const row = screen.getByRole('row')
    expect(row.className).toMatch(/hover:bg-muted\/50/)
    expect(row.className).not.toMatch(/hover:bg-destructive/)
  })

  it('a mandatory (non-error) row also keeps the standard hover unchanged', () => {
    renderRow(makeAiPoint(0, { mandatory_1815: true }))
    const row = screen.getByRole('row')
    expect(row.className).toMatch(/hover:bg-muted\/50/)
    expect(row.className).not.toMatch(/hover:bg-destructive/)
  })
})
