import { test, expect } from '@playwright/test'

const TOKEN = process.env.EF_TOKEN ?? ''
const GAME_ID = process.env.EF_GAME ?? ''
test.use({ baseURL: 'http://localhost:3000', viewport: { width: 1440, height: 1000 } })
test.setTimeout(60000)

function squarePoint(square: string, orientation: 'white' | 'black', box: { x: number; y: number; width: number; height: number }) {
  const file = square.charCodeAt(0) - 97
  const rank = Number(square[1])
  const size = box.width / 8
  const x = orientation === 'white' ? box.x + (file + 0.5) * size : box.x + (7 - file + 0.5) * size
  const y = orientation === 'white' ? box.y + (8 - rank + 0.5) * size : box.y + (rank - 1 + 0.5) * size
  return { x, y }
}

test('puzzle solve uses the board, sends an attempt and schedules it', async ({ page, context }) => {
  test.skip(!TOKEN, 'Set EF_TOKEN to a valid session token.')
  test.skip(!GAME_ID, 'Set EF_GAME to a game id owned by that session.')
  await context.addCookies([{ name: 'ef_session', value: TOKEN, domain: 'localhost', path: '/' }])
  const response = await context.request.get('http://localhost:3000/api/puzzles/due?limit=1')
  const puzzles = (await response.json()) as Array<{ fen: string; solution_uci: string }>
  test.skip(puzzles.length === 0, 'No due puzzle available.')
  const puzzle = puzzles[0]
  if (!puzzle) return
  const orientation = puzzle.fen.split(' ')[1] === 'b' ? 'black' : 'white'

  await page.goto('/puzzles', { waitUntil: 'domcontentloaded' })
  await page.waitForTimeout(1200)
  const box = await page.locator('cg-board').first().boundingBox()
  if (!box) throw new Error('board missing')
  const from = puzzle.solution_uci.slice(0, 2)
  const to = puzzle.solution_uci.slice(2, 4)
  const a = squarePoint(from, orientation, box)
  const b = squarePoint(to, orientation, box)
  await page.mouse.click(a.x, a.y)
  await page.waitForTimeout(150)
  await page.mouse.click(b.x, b.y)
  await page.waitForTimeout(300)
  await expect(page.locator('.puzzle-verdict')).toContainText(/correct|not the engine/i)
  await expect(page.locator('.puzzle-answer')).toBeVisible()
  await expect(page.locator('.puzzle-why')).toBeVisible()
  const good = page.getByRole('button', { name: /^Good/ })
  if (await good.count()) {
    await good.click()
    await expect(page.locator('.puzzle-result')).toContainText(/day interval/i)
  }
})

test('review opens after the move, shows a lesson and toggles the best move', async ({ page, context }) => {
  test.skip(!TOKEN, 'Set EF_TOKEN to a valid session token.')
  test.skip(!GAME_ID, 'Set EF_GAME to a game id owned by that session.')
  await context.addCookies([{ name: 'ef_session', value: TOKEN, domain: 'localhost', path: '/' }])
  await page.goto(`/games/${GAME_ID}`, { waitUntil: 'domcontentloaded' })
  await page.waitForTimeout(1200)
  await expect(page.locator('.lesson-panel')).toBeVisible()
  await expect(page.locator('.board-controls__label')).toContainText('after')
  await expect(page.getByText('WHAT HAPPENED')).toBeVisible()
  const lastMoveSquares = await page.locator('.cg-wrap cg-board square.last-move').count()
  expect(lastMoveSquares).toBeGreaterThan(1)
  await page.getByRole('button', { name: /show best move/i }).click()
  await page.waitForTimeout(250)
  await expect(page.locator('.board-controls__label')).toContainText('before')
})

test('focus report uses the browser timezone', async ({ page, context, browser }) => {
  test.skip(!TOKEN, 'Set EF_TOKEN to a valid session token.')
  const cairo = await browser.newContext({ baseURL: 'http://localhost:3000', timezoneId: 'Africa/Cairo' })
  await cairo.addCookies([{ name: 'ef_session', value: TOKEN, domain: 'localhost', path: '/' }])
  const cairoPage = await cairo.newPage()
  await cairoPage.goto('/focus', { waitUntil: 'domcontentloaded' })
  await cairoPage.waitForTimeout(1200)
  await expect(cairoPage.getByText(/Africa\/Cairo/i).first()).toBeVisible()
  await cairo.close()
  await page.waitForTimeout(50)
})

test('sparring accepts a legal board move and returns an engine reply', async ({ page, context }) => {
  test.skip(!TOKEN, 'Set EF_TOKEN to a valid session token.')
  await context.addCookies([{ name: 'ef_session', value: TOKEN, domain: 'localhost', path: '/' }])
  const response = await context.request.get('http://localhost:3000/api/sparring')
  const sessions = (await response.json()) as Array<{
    id: string
    fen: string
    status: string
    user_color: string
    moves: unknown[]
  }>
  const active = sessions.find((session) => session.status === 'active')
  test.skip(!active, 'No active sparring session available.')
  if (!active) return
  const { Chess } = await import('chess.js')
  const chess = new Chess(active.fen)
  const legal = chess.moves({ verbose: true })
  const move = legal[0]
  if (!move) return
  const afterUser = new Chess(active.fen)
  afterUser.move(move.san)
  const orientation = active.user_color === 'black' ? 'black' : 'white'

  await page.goto('/sparring', { waitUntil: 'domcontentloaded' })
  await page.waitForTimeout(1200)
  const hint = page.getByRole('button', { name: /coach hint/i })
  if (await hint.isEnabled()) {
    await hint.click()
    await expect(page.locator('.sparring-hint')).toBeVisible()
  }
  const box = await page.locator('cg-board').first().boundingBox()
  if (!box) throw new Error('board missing')
  const from = squarePoint(move.from, orientation, box)
  const to = squarePoint(move.to, orientation, box)
  const before = await page.locator('.sparring-moves li').count()
  await page.route('**/api/sparring/*/move', async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 900))
    await route.continue()
  })
  await page.mouse.click(from.x, from.y)
  await page.waitForTimeout(150)
  await page.mouse.click(to.x, to.y)
  await page.waitForTimeout(200)
  const optimistic = await page.locator('.board-frame').getAttribute('data-fen')
  expect(optimistic).toBe(afterUser.fen())
  await page.waitForTimeout(1800)
  const settled = await page.locator('.board-frame').getAttribute('data-fen')
  expect(settled).not.toBe(optimistic)
  await page.unroute('**/api/sparring/*/move')
  const after = await page.locator('.sparring-moves li').count()
  expect(after).toBeGreaterThan(before)
})

test('settings adopts the browser timezone and reports use it', async ({ browser }) => {
  test.skip(!TOKEN, 'Set EF_TOKEN to a valid session token.')
  const cairo = await browser.newContext({ baseURL: 'http://localhost:3000', timezoneId: 'Africa/Cairo' })
  await cairo.addCookies([{ name: 'ef_session', value: TOKEN, domain: 'localhost', path: '/' }])
  const page = await cairo.newPage()
  await page.goto('/settings', { waitUntil: 'domcontentloaded' })
  await page.waitForTimeout(1200)
  const timezone = page.getByLabel('Timezone')
  await timezone.selectOption('Africa/Cairo')
  await page.getByRole('button', { name: /save settings/i }).click()
  await expect(page.getByText(/Settings saved/i)).toBeVisible()
  const stored = await page.evaluate(() => window.localStorage.getItem('ef_timezone'))
  expect(stored).toBe('Africa/Cairo')
  await page.goto('/focus', { waitUntil: 'domcontentloaded' })
  await page.waitForTimeout(1200)
  await expect(page.getByText(/Africa\/Cairo/i).first()).toBeVisible()
  await cairo.close()
})

test('reflection dialog traps focus and returns it on close', async ({ page, context }) => {
  test.skip(!TOKEN, 'Set EF_TOKEN to a valid session token.')
  test.skip(!GAME_ID, 'Set EF_GAME to a game id owned by that session.')
  await context.addCookies([{ name: 'ef_session', value: TOKEN, domain: 'localhost', path: '/' }])
  await page.goto(`/games/${GAME_ID}`, { waitUntil: 'domcontentloaded' })
  await page.waitForTimeout(1200)
  await page.locator('.critical-row').first().click()
  const opener = page.getByRole('button', { name: /reflect on this move/i })
  await opener.click()
  const dialog = page.getByRole('dialog', { name: /why did i play this/i })
  await expect(dialog).toBeVisible()
  for (let index = 0; index < 6; index += 1) {
    await page.keyboard.press('Tab')
    const inside = await page.evaluate(() => {
      const drawer = document.querySelector('.drawer')
      return drawer ? drawer.contains(document.activeElement) : false
    })
    expect(inside).toBe(true)
  }
  await page.keyboard.press('Escape')
  await expect(dialog).toBeHidden()
  await expect(opener).toBeFocused()
})
