import { test, expect } from '@playwright/test'
import { TEST_PROFILE_NAME } from './shared_config'

test('add a curve and switch to the new curve', async ({ page }) => {
  await page.goto('/')
  // Silent bootstrap: assert on header profile name, not the toast.
  await expect(page.getByText(TEST_PROFILE_NAME)).toBeVisible({
    timeout: 10000,
  })
  await page.getByRole('tab', { name: 'Curves' }).click()
  await page.getByRole('button', { name: 'Add' }).click()
  await expect(page.getByText(`Curve 2`).first()).toBeAttached()
})
