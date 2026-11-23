import AxeBuilder from '@axe-core/playwright'
import { test } from '@playwright/test'

const TOKEN = process.env.EF_TOKEN ?? ''
const GAME_ID = process.env.EF_GAME ?? ''

test.use({ baseURL: 'http://localhost:3000', viewport: { width: 1440, height: 1000 } })
test.setTimeout(120000)

test('key pages have no serious accessibility violations', async ({ page, context }) => {
  test.skip(!TOKEN, 'Set EF_TOKEN to a valid session token to run this audit.')
  test.skip(!GAME_ID, 'Set EF_GAME to a game id owned by that session.')
  await context.addCookies([{ name: 'ef_session', value: TOKEN, domain: 'localhost', path: '/' }])

  const routes = ['/app', '/games', `/games/${GAME_ID}`, '/openings', '/focus', '/puzzles', '/sparring', '/roadmap', '/settings', '/more']
  for (const route of routes) {
    await page.goto(route, { waitUntil: 'domcontentloaded' })
    await page.waitForTimeout(1200)
    const results = await new AxeBuilder({ page })
      .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
      .analyze()
    const serious = results.violations.filter((violation) =>
      ['serious', 'critical'].includes(violation.impact ?? ''),
    )
    if (serious.length > 0) {
      console.log(`A11Y ${route}`)
      for (const violation of serious) {
        console.log(`- ${violation.id} (${violation.impact}): ${violation.help}`)
        for (const node of violation.nodes.slice(0, 4)) {
          console.log(`  ${node.target.join(' ')} :: ${node.failureSummary?.split('\n')[0] ?? ''}`)
        }
      }
    } else {
      console.log(`A11Y ${route} clean`)
    }
  }
})
