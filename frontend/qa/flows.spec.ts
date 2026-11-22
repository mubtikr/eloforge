import { test, expect } from '@playwright/test'

const TOKEN = process.env.EF_TOKEN ?? ''
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
  const good = page.getByRole('button', { name: /^Good/ })
  if (await good.count()) {
    await good.click()
    await expect(page.locator('.puzzle-result')).toContainText(/day interval/i)
  }
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
  const orientation = active.user_color === 'black' ? 'black' : 'white'

  await page.goto('/sparring', { waitUntil: 'domcontentloaded' })
  await page.waitForTimeout(1200)
  const box = await page.locator('cg-board').first().boundingBox()
  if (!box) throw new Error('board missing')
  const from = squarePoint(move.from, orientation, box)
  const to = squarePoint(move.to, orientation, box)
  const before = await page.locator('.sparring-moves li').count()
  await page.mouse.click(from.x, from.y)
  await page.waitForTimeout(150)
  await page.mouse.click(to.x, to.y)
  await page.waitForTimeout(1800)
  const after = await page.locator('.sparring-moves li').count()
  expect(after).toBeGreaterThan(before)
})
