import { test, expect } from '@playwright/test'
import { TEST_PROFILE_NAME } from './shared_config'

test('increase meter count, save, reload, and verify', async ({ page }) => {
  await page.goto('/')

  // Wait for the profile to auto-load from localStorage. Bootstrap is silent
  // (no toast on auto-load), so assert on the header showing the profile name.
  await expect(page.getByText(TEST_PROFILE_NAME)).toBeVisible({
    timeout: 10000,
  })

  await page.getByRole('tab', { name: 'Entities' }).click()

  // Find the Meters input via its description text
  const metersCard = page.getByText('Number of metering devices').locator('..')
  const metersInput = metersCard.getByRole('spinbutton')

  const currentValue = parseInt(await metersInput.inputValue())
  const newValue = currentValue + 2

  await metersInput.fill(String(newValue))
  await metersInput.blur()

  // Save the profile
  await page.getByRole('button', { name: 'Save' }).click()
  await expect(page.getByText(/saved successfully/i)).toBeVisible()

  // Reload the page
  await page.reload()

  // Wait for the profile to auto-load again (silent bootstrap, observe header)
  await expect(page.getByText(TEST_PROFILE_NAME)).toBeVisible({
    timeout: 10000,
  })

  await page.getByRole('tab', { name: 'Entities' }).click()

  const metersCardAfterReload = page
    .getByText('Number of metering devices')
    .locator('..')
  const metersInputAfterReload = metersCardAfterReload.getByRole('spinbutton')
  await expect(metersInputAfterReload).toHaveValue(String(newValue))
})
