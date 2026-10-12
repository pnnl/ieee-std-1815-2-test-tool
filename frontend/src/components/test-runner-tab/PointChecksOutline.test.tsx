import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import PointChecksOutline from './PointChecksOutline'
import type { PointCheckResult } from './types'

function check(
  pointType: 'AI' | 'AO',
  overrides: Partial<PointCheckResult> = {},
): PointCheckResult {
  return {
    event_type: 'point_check_result',
    scenario_id: 'scheduling',
    test: 'SCHED_002',
    group_name: 'Scheduling',
    point: {
      point_type: pointType,
      index: 3002,
      uid: 'FSCHn.SchdPrio',
      name: 'Selected Schedule Priority',
    },
    sequence: 1,
    timestamp: 0,
    check_id: 'profile_definition',
    expected_behavior: 'Defined in profile',
    passed: true,
    message: `${pointType} definition found`,
    ...overrides,
  }
}

function renderChecks(checks: PointCheckResult[]) {
  return render(
    <PointChecksOutline
      checks={checks}
      groupId="scheduling"
      label="Scheduling"
      isExpanded={() => true}
      onToggle={vi.fn()}
    />,
  )
}

describe('PointChecksOutline UID rows', () => {
  it('combines AI and AO checks and schedule readbacks under their shared UID', () => {
    renderChecks([
      check('AI'),
      check('AO', { sequence: 2 }),
      check('AI', {
        sequence: 3,
        check_id: 'Schedule_1_readback',
        expected_behavior: 'Schedule 1: Readback matches profile',
      }),
    ])

    const table = screen.getByRole('table', { name: 'Point tests' })
    expect(within(table).getAllByRole('row')).toHaveLength(2)
    const row = within(table).getAllByRole('row')[1]
    expect(within(row).getByText('FSCHn.SchdPrio')).toBeInTheDocument()
    expect(within(row).getByText('AI3002, AO3002')).toBeInTheDocument()
    expect(within(row).getAllByRole('button')).toHaveLength(3)
  })

  it('preserves a failing check when another index passes the same check', () => {
    renderChecks([
      check('AI', { passed: false, message: 'AI definition missing' }),
      check('AO', { sequence: 2 }),
    ])

    const ai = screen.getByRole('button', {
      name: 'AI3002: Defined in profile: Failed. Hide details',
    })
    const ao = screen.getByRole('button', {
      name: 'AO3002: Defined in profile: Passed. Show details',
    })
    expect(screen.getByText('AI definition missing')).toBeInTheDocument()
    fireEvent.click(ao)
    expect(screen.getByText('AO definition found')).toBeInTheDocument()
    fireEvent.click(ai)
    expect(screen.queryByText('AI definition missing')).not.toBeInTheDocument()
    expect(screen.getByText('AO definition found')).toBeInTheDocument()
  })

  it('keeps different UIDs separate and uses the latest result per index and check', () => {
    const old = check('AI', { passed: false, message: 'Stale failure' })
    const latest = check('AI', { sequence: 3 })
    const other = check('AO', {
      point: { ...check('AO').point, uid: 'FSCHn.NumEntr', index: 3009 },
      sequence: 2,
    })
    renderChecks([latest, other, old])

    const table = screen.getByRole('table', { name: 'Point tests' })
    expect(within(table).getAllByRole('row')).toHaveLength(3)
    expect(within(table).getAllByRole('button')).toHaveLength(2)
    expect(screen.queryByText('Stale failure')).not.toBeInTheDocument()
    expect(
      screen.getByRole('button', {
        name: 'AI3002: Defined in profile: Passed. Show details',
      }),
    ).toBeInTheDocument()
  })
})
