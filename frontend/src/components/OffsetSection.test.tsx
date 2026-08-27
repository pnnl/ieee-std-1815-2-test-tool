// Tests for the group-header validation error badge. Mirrors
// Tabs.test.tsx's badge assertions one level down, plus the badge staying
// visible while the group is collapsed, the case the feature exists for.

import { describe, it, expect } from 'vitest'
import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import OffsetSection from './OffsetSection'
import type { FlatAiPoint } from '@/profile/canonical'
import type { ValidationError } from '@/api/generated'
import { renderWithTheme } from '@/testUtils'

function makeAiPoint(pointIndex: number): FlatAiPoint {
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
  }
}

describe('OffsetSection group error badge', () => {
  it('renders no badge when errorCount is omitted', () => {
    renderWithTheme(
      <OffsetSection
        offsetName="scada"
        offsetValue={0}
        points={[makeAiPoint(0)]}
        tabName="analog_inputs"
        onPointUpdate={() => {}}
      />,
    )
    const header = screen.getByRole('button', { name: /Scada/ })
    expect(header.querySelector('[data-slot="badge"]')).toBeNull()
  })

  it('renders no badge when errorCount is exactly zero', () => {
    renderWithTheme(
      <OffsetSection
        offsetName="scada"
        offsetValue={0}
        points={[makeAiPoint(0)]}
        tabName="analog_inputs"
        onPointUpdate={() => {}}
        errorCount={0}
      />,
    )
    const header = screen.getByRole('button', { name: /Scada/ })
    expect(header.querySelector('[data-slot="badge"]')).toBeNull()
  })

  it('renders the exact count, visible and accessible, for a group with errors', () => {
    renderWithTheme(
      <OffsetSection
        offsetName="scada"
        offsetValue={0}
        points={[makeAiPoint(0), makeAiPoint(2), makeAiPoint(3)]}
        tabName="analog_inputs"
        onPointUpdate={() => {}}
        errorCount={5}
      />,
    )
    const header = screen.getByRole('button', { name: /Scada/ })
    expect(header.querySelector('[aria-hidden="true"]')?.textContent).toBe('5')
    expect(header).toHaveTextContent('5 validation errors')
  })

  it('uses the singular "error" for a count of exactly one', () => {
    renderWithTheme(
      <OffsetSection
        offsetName="scada"
        offsetValue={0}
        points={[makeAiPoint(0)]}
        tabName="analog_inputs"
        onPointUpdate={() => {}}
        errorCount={1}
      />,
    )
    const header = screen.getByRole('button', { name: /Scada/ })
    expect(header).toHaveTextContent('1 validation error')
    expect(header).not.toHaveTextContent('1 validation errors')
  })

  // The invariant the feature exists for: a collapsed group must not hide
  // its own badge. autoExpand is unset, so the section renders collapsed
  // by default, and this test never clicks to expand it.
  it('shows the badge on a collapsed group, without expanding it', () => {
    renderWithTheme(
      <OffsetSection
        offsetName="scada"
        offsetValue={0}
        points={[makeAiPoint(0), makeAiPoint(2)]}
        tabName="analog_inputs"
        onPointUpdate={() => {}}
        errorCount={2}
      />,
    )

    // Confirm it starts collapsed: the table body isn't present.
    expect(screen.queryByRole('table')).not.toBeInTheDocument()

    // The badge is outside CollapsibleContent, so it's present regardless
    // of expand state.
    const header = screen.getByRole('button', { name: /Scada/ })
    expect(header.querySelector('[aria-hidden="true"]')?.textContent).toBe('2')
    expect(header).toHaveTextContent('2 validation errors')
  })
})

// Wiring check: OffsetSection forwards `errorsByPointIndex` down to the
// right PointRow by point_index, the same lookup the group badge uses.
describe('OffsetSection row highlight wiring', () => {
  it('highlights only the rows whose point index is a key in errorsByPointIndex', async () => {
    const errorsByPointIndex = new Map<number, ValidationError[]>([
      [
        0,
        [
          {
            point: 'AI0',
            message: 'Value must be between 1 and 100, but is 999',
          },
        ],
      ],
      [
        2,
        [
          {
            point: 'AI2',
            message: 'Minimum value cannot be greater than maximum value',
          },
          { point: 'AI2', message: 'Value must be between 10 and 0, but is 0' },
        ],
      ],
    ])

    renderWithTheme(
      <OffsetSection
        offsetName="scada"
        offsetValue={0}
        points={[
          makeAiPoint(0),
          makeAiPoint(1),
          makeAiPoint(2),
          makeAiPoint(3),
        ]}
        tabName="analog_inputs"
        onPointUpdate={() => {}}
        errorCount={3}
        errorsByPointIndex={errorsByPointIndex}
      />,
    )

    // Expand to reach the rows.
    await userEvent.click(screen.getByRole('button', { name: /Scada/ }))

    const rows = screen.getAllByRole('row')
    // Header row + 4 point rows.
    expect(rows).toHaveLength(5)

    const ai0Row = screen.getByText('Point 0').closest('tr')!
    const ai1Row = screen.getByText('Point 1').closest('tr')!
    const ai2Row = screen.getByText('Point 2').closest('tr')!
    const ai3Row = screen.getByText('Point 3').closest('tr')!

    expect(ai0Row.className).toMatch(/bg-destructive\/10/)
    expect(ai2Row.className).toMatch(/bg-destructive\/10/)
    // Neither AI1 nor AI3 has an entry in errorsByPointIndex: unstyled.
    expect(ai1Row.className).not.toMatch(/destructive/)
    expect(ai3Row.className).not.toMatch(/destructive/)

    // AI2's two errors produce one row with both messages, not two rows.
    expect(ai2Row).toHaveAttribute(
      'title',
      'Minimum value cannot be greater than maximum value; Value must be between 10 and 0, but is 0',
    )
  })

  it('renders no highlighted rows when errorsByPointIndex is omitted', async () => {
    renderWithTheme(
      <OffsetSection
        offsetName="scada"
        offsetValue={0}
        points={[makeAiPoint(0), makeAiPoint(1)]}
        tabName="analog_inputs"
        onPointUpdate={() => {}}
      />,
    )
    await userEvent.click(screen.getByRole('button', { name: /Scada/ }))
    for (const row of screen.getAllByRole('row')) {
      expect(row.className).not.toMatch(/destructive/)
    }
  })
})
