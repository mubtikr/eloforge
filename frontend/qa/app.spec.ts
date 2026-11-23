import { test, expect } from '@playwright/test'

const TOKEN = process.env.EF_TOKEN ?? ''
const GAME_ID = process.env.EF_GAME ?? ''

test.use({ baseURL: 'http://localhost:3000', viewport: { width: 1440, height: 1000 } })
test.setTimeout(120000)

test('study room pages render without runtime errors', async ({ page, context }) => {
  test.skip(!TOKEN, 'Set EF_TOKEN to a valid session token to run this smoke test.')
  test.skip(!GAME_ID, 'Set EF_GAME to a game id owned by that session.')
  await context.addCookies([{ name: 'ef_session', value: TOKEN, domain: 'localhost', path: '/' }])
  const errors: string[] = []
  const failures: string[] = []
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text())
  })
  page.on('pageerror', (error) => errors.push(error.message))
  page.on('response', (response) => {
    if (response.status() >= 400) failures.push(`${response.status()} ${response.url()}`)
  })

  const routes = [
    ['/app', 'dashboard'],
    ['/games', 'games'],
    ['/more', 'more'],
    ['/openings', 'openings'],
    ['/focus', 'focus'],
    ['/puzzles', 'puzzles'],
    ['/sparring', 'sparring'],
    ['/roadmap', 'roadmap'],
    ['/settings', 'settings'],
  ] as const

  for (const [route, name] of routes) {
    await page.goto(route, { waitUntil: 'domcontentloaded' })
    await page.waitForTimeout(700)
    const h1 = await page.locator('h1').first().innerText()
    console.log(name, '=>', h1.replace(/\n/g, ' '))
    await page.screenshot({ path: `/tmp/shots/${name}.png`, fullPage: true })
  }

  if (GAME_ID) {
    await page.goto(`/games/${GAME_ID}`, { waitUntil: 'domcontentloaded' })
    await page.waitForTimeout(900)
    console.log('match =>', (await page.locator('h1').first().innerText()).replace(/\n/g, ' '))
    await page.screenshot({ path: '/tmp/shots/match.png', fullPage: true })
    const badMove = page.locator('.critical-row').first()
    if (await badMove.count()) {
      await badMove.click()
      await page.waitForTimeout(300)
      const reflect = page.getByRole('button', { name: /reflect on this move/i }).first()
      if (await reflect.count()) {
        await reflect.click()
        await page.waitForTimeout(300)
        await page.screenshot({ path: '/tmp/shots/reflect.png', fullPage: false })
        await page.keyboard.press('Escape')
      }
    }
  }

  await page.goto('/', { waitUntil: 'domcontentloaded' })
  await page.waitForTimeout(500)
  console.log('landing =>', (await page.locator('h1').first().innerText()).replace(/\n/g, ' '))
  await page.screenshot({ path: '/tmp/shots/landing.png', fullPage: true })

  await page.setViewportSize({ width: 390, height: 844 })
  const mobileRoutes = [
    ['/app', 'dashboard-mobile'],
    ['/games', 'games-mobile'],
    [`/games/${GAME_ID}`, 'match-mobile'],
    ['/puzzles', 'puzzles-mobile'],
    ['/sparring', 'sparring-mobile'],
    ['/focus', 'focus-mobile'],
    ['/openings', 'openings-mobile'],
    ['/roadmap', 'roadmap-mobile'],
    ['/settings', 'settings-mobile'],
    ['/more', 'more-mobile'],
  ] as const
  for (const [route, name] of mobileRoutes) {
    await page.goto(route, { waitUntil: 'domcontentloaded' })
    await page.waitForTimeout(900)
    const board = page.locator('cg-board').first()
    if (await board.count()) {
      const box = await board.boundingBox()
      if (!box || box.width > 390) throw new Error(`${name}: board overflow ${box?.width}`)
    }
    await page.screenshot({ path: `/tmp/shots/${name}.png`, fullPage: true })
  }

  console.log('ERRORS', JSON.stringify(errors, null, 2))
  console.log('HTTP_FAILURES', JSON.stringify(failures, null, 2))
  expect(failures).toEqual([])
  expect(errors).toEqual([])
})
