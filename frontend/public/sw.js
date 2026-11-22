self.addEventListener('push', (event) => {
  let payload = {}
  try {
    payload = event.data ? event.data.json() : {}
  } catch {
    payload = { title: 'EloForge', body: event.data ? event.data.text() : '' }
  }
  event.waitUntil(
    self.registration.showNotification(payload.title || 'EloForge', {
      body: payload.body || 'Your study queue is ready.',
      tag: 'eloforge',
      data: { url: '/app' },
    }),
  )
})

self.addEventListener('notificationclick', (event) => {
  event.notification.close()
  const target = event.notification.data?.url || '/app'
  event.waitUntil(
    clients.matchAll({ type: 'window', includeUncontrolled: true }).then((windows) => {
      for (const windowClient of windows) {
        if ('focus' in windowClient) {
          windowClient.navigate(target)
          return windowClient.focus()
        }
      }
      return clients.openWindow(target)
    }),
  )
})
