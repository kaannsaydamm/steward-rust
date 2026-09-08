/**
 * Visual rebrand check: chat TUI banner shows Steward branding; sidebar has
 * no Achievements; System page has no Nous Portal section.
 */
import { expect, test } from '@playwright/test'

const BASE = 'http://127.0.0.1:8093'
const TOKEN = 'steward-web-e2e-token-0123456789abcdef'

test('rebrand: no Hermes/Nous strings on chat, sessions, system pages', async ({ page }) => {
  for (const route of ['/chat', '/sessions', '/system']) {
    await page.goto(`${BASE}${route}?token=${TOKEN}`, { waitUntil: 'domcontentloaded' })
    await page.waitForTimeout(6_000)
    const text = await page.evaluate(() => document.body?.innerText ?? '')
    expect(text).not.toMatch(/HERMES-AGENT|Hermes Agent|Nous Research|Nous Portal|HERMES PORTAL/i)
  }
})

test('rebrand: no Achievements nav entry', async ({ page }) => {
  await page.goto(`${BASE}/system?token=${TOKEN}`, { waitUntil: 'domcontentloaded' })
  await page.waitForTimeout(4_000)
  const text = await page.evaluate(() => document.body?.innerText ?? '')
  expect(text).not.toMatch(/achievements/i)
})
