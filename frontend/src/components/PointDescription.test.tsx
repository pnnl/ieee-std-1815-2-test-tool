import { describe, expect, it } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import PointDescription from './PointDescription'

function mapping(count: number) {
  return `Mode. ${Array.from({ length: count }, (_, index) => `<${index}> Label ${index}`).join(' ')}`
}

describe('PointDescription', () => {
  it('renders short mappings as semantic value/label pairs', () => {
    const { container } = render(<PointDescription text={mapping(8)} />)
    expect(screen.getByText('Mode.')).toBeInTheDocument()
    expect(container.querySelectorAll('dt')).toHaveLength(8)
    expect(container.querySelectorAll('dd')).toHaveLength(8)
    expect(container.querySelector('details')).toBeNull()
    expect(screen.getByText('Label 7')).toBeVisible()
  })

  it('starts long mappings collapsed with a visible introduction and count', () => {
    const { container } = render(<PointDescription text={mapping(9)} />)
    const details = container.querySelector('details')!
    expect(details.open).toBe(false)
    expect(screen.getByText('Mode.')).toBeVisible()
    const summary = screen.getByText('Enumeration (9 entries)')
    expect(summary.tagName).toBe('SUMMARY')
    fireEvent.click(summary)
    expect(details.open).toBe(true)
    expect(screen.getByText('Label 8')).toBeVisible()
    fireEvent.click(summary)
    expect(details.open).toBe(false)
  })

  it('keeps defaults visible without introducing duplicate definitions', () => {
    const { container } = render(
      <PointDescription text="Reference. Default is <3>. <0> Unknown <3> Available" />,
    )
    expect(screen.getByText('Reference. Default is <3>.')).toBeVisible()
    expect(container.querySelectorAll('dt')).toHaveLength(2)
  })

  it('keeps notes and headings between definitions', () => {
    const { container } = render(
      <PointDescription
        text={'Mode.\n<0> Off\nHeading\n<1> On ... <234+> Other'}
      />,
    )
    expect(container.textContent).toBe('Mode.0OffHeading1On... <234+> Other')
  })

  it('renders all descriptions as text, never injected HTML', () => {
    const { container } = render(
      <PointDescription
        text={'Mode. <0> Off <img src=x onerror=alert(1)> <1> On'}
      />,
    )
    expect(container.querySelector('img')).toBeNull()
    expect(
      screen.getByText('Off <img src=x onerror=alert(1)>'),
    ).toBeInTheDocument()
  })

  it('leaves plain text unchanged', () => {
    const { container } = render(
      <PointDescription text={'Default is <3>. &lt;img&gt;'} />,
    )
    expect(container.textContent).toBe('Default is <3>. &lt;img&gt;')
    expect(container.querySelector('dl')).toBeNull()
  })
})
