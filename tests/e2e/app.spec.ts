import { test, expect } from '@playwright/test';

test.describe('Cold-Brew App', () => {
	test('app loads and shows navigation', async ({ page }) => {
		await page.goto('http://localhost:5173');

		await expect(page).toHaveTitle(/Cold-Brew/i);

		await expect(page.locator('nav a, nav button, [data-nav]')).not.toHaveCount(0);
	});

	test('sidebar navigation links are present', async ({ page }) => {
		await page.goto('http://localhost:5173');

		const navLinks = page.locator("nav a, [role='navigation'] a, [data-nav-link]");
		const count = await navLinks.count();

		expect(count).toBeGreaterThanOrEqual(1);
	});

	test('player route is accessible', async ({ page }) => {
		await page.goto('http://localhost:5173/player');

		await expect(page).toHaveTitle(/Cold-Brew/i);
	});

	test('settings route is accessible', async ({ page }) => {
		await page.goto('http://localhost:5173/settings');

		await expect(page).toHaveTitle(/Cold-Brew/i);
	});
});
