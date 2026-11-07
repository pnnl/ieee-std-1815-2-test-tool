import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import TestResultsTree from './TestResultsTree'
import type { Scenario } from './scenarios'
import type { ConformanceTestResult } from './types'

const scenarios: Scenario[] = [
  {
    id: 'configuration',
    name: 'Configuration',
    description: 'Configuration tests',
    expected_tests: [],
  },
  {
    id: 'monitoring',
    name: 'Monitoring',
    description: 'Monitoring tests',
    expected_tests: [],
  },
]

function renderTree(
  enabledScenarios: Record<string, boolean>,
  onSetAllScenarios = vi.fn(),
) {
  render(
    <TestResultsTree
      scenarios={scenarios}
      results={{}}
      hasStarted={false}
      enabledScenarios={enabledScenarios}
      onToggleScenario={vi.fn()}
      onSetAllScenarios={onSetAllScenarios}
    />,
  )
}

describe('TestResultsTree bulk selection actions', () => {
  it('shows only Deselect all when every scenario is selected', () => {
    renderTree({ configuration: true, monitoring: true })

    const clearAll = screen.getByRole('button', { name: 'Deselect all' })
    expect(clearAll).toBeInTheDocument()
    expect(clearAll.querySelector('svg')).toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: 'Select all' }),
    ).not.toBeInTheDocument()
  })

  it('shows only Select all when every scenario is cleared', () => {
    renderTree({ configuration: false, monitoring: false })

    const selectAll = screen.getByRole('button', { name: 'Select all' })
    expect(selectAll).toBeInTheDocument()
    expect(selectAll.querySelector('svg')).toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: 'Deselect all' }),
    ).not.toBeInTheDocument()
  })

  it('shows both actions for a partial selection and reports each choice', () => {
    const onSetAllScenarios = vi.fn()
    renderTree({ configuration: true, monitoring: false }, onSetAllScenarios)

    fireEvent.click(screen.getByRole('button', { name: 'Deselect all' }))
    fireEvent.click(screen.getByRole('button', { name: 'Select all' }))

    expect(onSetAllScenarios).toHaveBeenNthCalledWith(1, false)
    expect(onSetAllScenarios).toHaveBeenNthCalledWith(2, true)
  })
})

describe('TestResultsTree applicability notes', () => {
  const note = 'Bounds check N/A: no eligible AO points.'
  const boundsScenario: Scenario = {
    id: 'lower_bounds',
    name: 'Lower Bounds',
    description: 'Lower bounds tests',
    expected_tests: [
      { test_id: 'MON_001', should_pass: true, applicability_note: note },
      { test_id: 'CVAR_001', should_pass: false },
    ],
  }

  it.each([undefined, true, false])(
    'shows the note and preserves status and counts when passed=%s',
    async (passed) => {
      const results: Record<string, ConformanceTestResult> =
        passed === undefined
          ? {}
          : {
              'lower_bounds:MON_001': {
                event_type: 'conformance_test_result',
                test: 'MON_001',
                scenario_id: 'lower_bounds',
                timestamp: 1000,
                passed,
                comments: [],
              },
            }
      render(
        <TestResultsTree
          scenarios={[boundsScenario]}
          results={results}
          hasStarted={passed !== undefined}
          enabledScenarios={{ lower_bounds: true }}
          onToggleScenario={vi.fn()}
          onSetAllScenarios={vi.fn()}
        />,
      )

      const scenarioRow = screen.getByRole('treeitem', { name: 'Lower Bounds' })
      fireEvent.click(scenarioRow)
      expect(await screen.findByText(note)).toBeInTheDocument()
      const monRow = screen.getByRole('treeitem', { name: 'MON_001' })
      const cvarRow = screen.getByRole('treeitem', { name: 'CVAR_001' })
      expect(monRow).not.toHaveTextContent('xfail')
      expect(cvarRow).toHaveTextContent('xfail')
      expect(cvarRow).not.toHaveTextContent(note)
      if (passed !== undefined) {
        const icon = passed ? '✓' : '✕'
        expect(monRow).toHaveTextContent(icon)
        expect(scenarioRow).toHaveTextContent(`${icon}1`)
      }
    },
  )
})
