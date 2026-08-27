import { test, expect } from '@playwright/test'
import { TEST_PROFILE_NAME } from './shared_config'

test('add a schedule and verify Schedule 0 appears', async ({ page }) => {
  await page.goto('/')
  // Silent bootstrap: assert on header profile name, not the toast.
  await expect(page.getByText(TEST_PROFILE_NAME)).toBeVisible({
    timeout: 10000,
  })
  await page.getByRole('tab', { name: 'Scheduling' }).click()
  await page.getByRole('button', { name: '+ Add Schedule' }).click()
  await expect(page.getByText('Schedule 0').first()).toBeVisible()
})
