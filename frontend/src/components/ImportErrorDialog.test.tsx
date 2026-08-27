import { describe, it, expect, vi } from 'vitest'
import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import ImportErrorDialog from './ImportErrorDialog'
import type { ValidationError } from '@/api/generated'
import { renderWithTheme } from '@/testUtils'

describe('ImportErrorDialog', () => {
  it('is not present in the DOM when closed', () => {
    renderWithTheme(
      <ImportErrorDialog open={false} errors={[]} onClose={() => {}} />,
    )
    expect(screen.queryByText('Import failed')).not.toBeInTheDocument()
  })

  it('lists every error and states the file was not loaded', () => {
    const errors: ValidationError[] = [
      { point: 'N/A', message: 'bad header row' },
      { point: 'Key', message: 'missing Key sheet' },
    ]
    renderWithTheme(
      <ImportErrorDialog open={true} errors={errors} onClose={() => {}} />,
    )

    expect(screen.getByText('Import failed')).toBeInTheDocument()
    expect(screen.getByText(/was not loaded/)).toBeInTheDocument()
    expect(screen.getByText(/bad header row/)).toBeInTheDocument()
    expect(screen.getByText(/missing Key sheet/)).toBeInTheDocument()
  })

  it('calls onClose exactly once when the Close button is clicked (Copilot review finding on #526: Dialog.Close and an explicit onClick both fired it)', async () => {
    const onClose = vi.fn()
    const user = userEvent.setup()
    renderWithTheme(
      <ImportErrorDialog
        open={true}
        errors={[{ point: '', message: 'raw text failure' }]}
        onClose={onClose}
      />,
    )

    await user.click(screen.getByRole('button', { name: 'Close' }))
    expect(onClose).toHaveBeenCalledTimes(1)
  })
})
