export interface SseFrame {
  event: string
  data: unknown
}

export async function streamSse(
  path: string,
  body: unknown,
  onFrame: (frame: SseFrame) => void,
  signal?: AbortSignal,
): Promise<void> {
  const response = await fetch(path, {
    method: 'POST',
    credentials: 'include',
    headers: { 'content-type': 'application/json', accept: 'text/event-stream' },
    body: JSON.stringify(body),
    signal,
  })

  if (!response.ok) {
    let message = `Reflection failed (${response.status})`
    try {
      const parsed = (await response.json()) as { error?: { message?: string } }
      if (parsed.error?.message) message = parsed.error.message
    } catch {
      // keep status message
    }
    throw new Error(message)
  }

  const reader = response.body?.getReader()
  if (!reader) throw new Error('Streaming is not available in this browser.')
  const decoder = new TextDecoder()
  let buffer = ''

  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })
    for (;;) {
      const index = buffer.indexOf('\n\n')
      if (index === -1) break
      const raw = buffer.slice(0, index)
      buffer = buffer.slice(index + 2)
      let event = 'message'
      const dataLines: string[] = []
      for (const line of raw.split('\n')) {
        const clean = line.endsWith('\r') ? line.slice(0, -1) : line
        if (clean.startsWith('event:')) event = clean.slice(6).trim()
        else if (clean.startsWith('data:')) dataLines.push(clean.slice(5).trimStart())
      }
      if (dataLines.length === 0) continue
      const payload = dataLines.join('\n')
      let data: unknown = payload
      try {
        data = JSON.parse(payload)
      } catch {
        // plain text frame
      }
      onFrame({ event, data })
    }
  }
}
