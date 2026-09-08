import { describe, it, expect } from 'vitest'
import { screen } from '@testing-library/react'
import ValidationErrorCallout from './ValidationErrorCallout'
import type { ValidationError } from '@/api/generated'
import { renderWithTheme } from '@/testUtils'

describe('ValidationErrorCallout', () => {
  it('renders nothing for an empty error list', () => {
    const { container } = renderWithTheme(
      <ValidationErrorCallout errors={[]} />,
    )
    // The Theme wrapper itself renders a div with data attributes, so assert
    // no child content rather than an empty container.
    expect(container.textContent).toBe('')
    expect(
      container.querySelector('[role="alert"], [class*="Callout"]'),
    ).toBeNull()
  })

  it('renders every error message', () => {
    const errors: ValidationError[] = [
      { point: 'AI12', message: 'value out of range' },
      { point: '', message: 'workbook missing Key sheet' },
    ]
    renderWithTheme(<ValidationErrorCallout errors={errors} />)

    expect(screen.getByText(/value out of range/)).toBeInTheDocument()
    expect(screen.getByText(/workbook missing Key sheet/)).toBeInTheDocument()
    expect(screen.getByText(/AI12:/)).toBeInTheDocument()
  })

  it('renders an optional title', () => {
    renderWithTheme(
      <ValidationErrorCallout
        errors={[{ point: 'AI1', message: 'bad' }]}
        title="Profile validation issues"
      />,
    )
    expect(screen.getByText('Profile validation issues')).toBeInTheDocument()
  })

  it('review finding 1: shows a count badge next to the title, derived from errors.length', () => {
    const errors: ValidationError[] = [
      { point: 'N/A', message: 'workbook structural fault one' },
      { point: 'Key', message: 'workbook structural fault two' },
      { point: '', message: 'workbook structural fault three' },
    ]
    renderWithTheme(
      <ValidationErrorCallout
        errors={errors}
        title="Profile validation issues"
      />,
    )

    expect(screen.getByText('Profile validation issues')).toBeInTheDocument()
    // The visible digit and the accessible text are asserted separately:
    // the digit for sighted users, the sr-only text for screen readers, per
    // the invariant that the count is not conveyed by position alone.
    expect(
      screen.getByText('3', { selector: '[aria-hidden="true"]' }),
    ).toBeInTheDocument()
    expect(
      screen.getByText('3 validation errors', { exact: false }),
    ).toBeInTheDocument()
  })

  it('review finding 1: the count badge is singular for exactly one error', () => {
    renderWithTheme(
      <ValidationErrorCallout
        errors={[{ point: 'AI1', message: 'bad' }]}
        title="Profile validation issues"
      />,
    )
    expect(
      screen.getByText('1 validation error', { exact: false }),
    ).toBeInTheDocument()
  })

  it('review finding 1: no title means no count badge is rendered (per-tab callout usage)', () => {
    const { container } = renderWithTheme(
      <ValidationErrorCallout errors={[{ point: 'AI1', message: 'bad' }]} />,
    )
    expect(container.querySelector('[data-slot="badge"]')).toBeNull()
  })

  it('The title-badge markup is the shared ValidationErrorBadge, not a re-implemented copy', () => {
    const { container } = renderWithTheme(
      <ValidationErrorCallout
        errors={[
          { point: 'AI1', message: 'bad' },
          { point: 'AI2', message: 'also bad' },
        ]}
        title="Profile validation issues"
      />,
    )
    const badge = container.querySelector('[data-slot="badge"]')
    expect(badge).not.toBeNull()
    expect(badge).toHaveClass(
      'h-4',
      'min-w-4',
      'justify-center',
      'rounded-full',
      'px-1',
      'text-[10px]',
      'leading-4',
    )
  })
})
