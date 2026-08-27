import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import TestResultsTree from './TestResultsTree'
import type { Scenario } from './scenarios'

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
