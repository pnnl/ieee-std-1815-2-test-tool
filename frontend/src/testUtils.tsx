// Shared component-test render helper.

import type { ReactElement } from 'react'
import { render } from '@testing-library/react'
import { Theme } from '@radix-ui/themes'

export function renderWithTheme(ui: ReactElement) {
  return render(<Theme>{ui}</Theme>)
}
