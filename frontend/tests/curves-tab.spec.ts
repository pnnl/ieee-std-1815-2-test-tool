import { test, expect } from '@playwright/test'
import { TEST_PROFILE_NAME } from './shared_config'

test('resize an empty curve to one point and persist the returned pair', async ({
  page,
}) => {
  await page.goto('/')
  await expect(page.getByText(TEST_PROFILE_NAME)).toBeVisible({
    timeout: 10000,
  })
  await page.getByRole('tab', { name: 'Curves', exact: true }).click()
  const count = page.getByRole('spinbutton', { name: 'Number of Points' })
  await count.fill('0')
  await count.press('Tab')
  await expect(page.getByText('0 active', { exact: true })).toBeVisible()
  await expect(count).toBeEnabled()

  const resized = page.waitForResponse(
    (response) =>
      response.url().endsWith('/api/profiles/curves/points') &&
      response.request().method() === 'POST',
  )
  await count.fill('1')
  await count.press('Tab')
  await expect(count).toHaveValue('1')
  const response = await resized
  expect(response.ok()).toBeTruthy()
  const { profile } = await response.json()
  const curve = profile.AI.curves[0]
  expect(curve.number_of_points.value).toBe(1)
  expect(curve.x_values).toHaveLength(1)
  expect(curve.y_values).toHaveLength(1)
  expect(curve.x_values[0].point_index).toBe(curve.y_units.point_index + 1)
  expect(curve.y_values[0].point_index).toBe(curve.y_units.point_index + 2)
  await expect(page.getByText('1 active', { exact: true })).toBeVisible()
  await expect(page.locator('tbody input[type="number"]')).toHaveCount(2)

  const saved = page.waitForResponse(
    (r) => r.url().endsWith('/api/profiles') && r.request().method() === 'POST',
  )
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  expect((await saved).ok()).toBeTruthy()
  await page.reload()
  await page.getByRole('tab', { name: 'Curves', exact: true }).click()
  await expect(count).toHaveValue('1')
  await expect(page.locator('tbody input[type="number"]')).toHaveCount(2)
})

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
