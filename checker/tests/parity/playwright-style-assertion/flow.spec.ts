import { expect, test } from '@playwright/test';

test('menu', async ({ page }) => {
  await expect(page.locator('nav')).toHaveCSS('display', 'flex');
  await expect(page.getByRole('link')).toBeVisible();
});
