// Tests for the per-tab validation error count badges: a tab with no
// errors renders no badge, a tab with errors renders a badge whose count
// is available to a screen reader, not conveyed by the digit alone.

import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import Tabs from './Tabs'

const TABS = [
  { id: 'entities', label: 'Entities' },
  { id: 'binary_outputs', label: 'Binary Outputs' },
  { id: 'binary_inputs', label: 'Binary Inputs' },
  { id: 'analog_outputs', label: 'Analog Outputs' },
  { id: 'analog_inputs', label: 'Analog Inputs' },
]

describe('Tabs error count badges', () => {
  it('renders no badge for a tab with a zero (or absent) count', () => {
    render(
      <Tabs
        tabs={TABS}
        activeTab="entities"
        onTabChange={() => {}}
        errorCounts={{ analog_inputs: 3 }}
      />,
    )

    const binaryOutputsTab = screen.getByRole('tab', {
      name: /^Binary Outputs/,
    })
    expect(binaryOutputsTab.querySelector('[data-slot="badge"]')).toBeNull()
  })

  it('renders no badge at all when errorCounts is omitted entirely', () => {
    render(<Tabs tabs={TABS} activeTab="entities" onTabChange={() => {}} />)

    for (const tab of TABS) {
      const trigger = screen.getByRole('tab', {
        name: new RegExp(`^${tab.label}`),
      })
      expect(trigger.querySelector('[data-slot="badge"]')).toBeNull()
    }
  })

  it('renders the exact count for a tab with errors, visible and accessible', () => {
    render(
      <Tabs
        tabs={TABS}
        activeTab="entities"
        onTabChange={() => {}}
        errorCounts={{ analog_inputs: 3, binary_outputs: 0 }}
      />,
    )

    const analogInputsTab = screen.getByRole('tab', {
      name: /^Analog Inputs/,
    })
    expect(
      analogInputsTab.querySelector('[aria-hidden="true"]')?.textContent,
    ).toBe('3')
    expect(analogInputsTab).toHaveTextContent('3 validation errors')

    const binaryOutputsTab = screen.getByRole('tab', {
      name: /^Binary Outputs/,
    })
    expect(binaryOutputsTab.querySelector('[data-slot="badge"]')).toBeNull()
  })

  it('uses the singular "error" for a count of exactly one', () => {
    render(
      <Tabs
        tabs={TABS}
        activeTab="entities"
        onTabChange={() => {}}
        errorCounts={{ analog_inputs: 1 }}
      />,
    )

    const analogInputsTab = screen.getByRole('tab', {
      name: /^Analog Inputs/,
    })
    expect(analogInputsTab).toHaveTextContent('1 validation error')
    expect(analogInputsTab).not.toHaveTextContent('1 validation errors')
  })
})
