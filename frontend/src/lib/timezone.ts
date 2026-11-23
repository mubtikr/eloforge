const STORAGE_KEY = 'ef_timezone'

export function browserTimeZone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'
  } catch {
    return 'UTC'
  }
}

export function getDisplayTimeZone(): string {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY)
    if (stored) return stored
  } catch {
    // localStorage can be unavailable in private modes
  }
  return browserTimeZone()
}

export function setDisplayTimeZone(timeZone: string): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, timeZone)
  } catch {
    // ignore storage failures
  }
}

export function timeZoneLabel(timeZone: string): string {
  try {
    const formatter = new Intl.DateTimeFormat('en-GB', {
      timeZone,
      timeZoneName: 'shortOffset',
    })
    const part = formatter.formatToParts(new Date()).find((item) => item.type === 'timeZoneName')
    return part?.value ? `${timeZone} (${part.value})` : timeZone
  } catch {
    return timeZone
  }
}

export function hourInTimeZone(value: Date, timeZone: string): number {
  try {
    const part = new Intl.DateTimeFormat('en-GB', {
      timeZone,
      hour: 'numeric',
      hourCycle: 'h23',
    })
      .formatToParts(value)
      .find((item) => item.type === 'hour')
    const parsed = Number(part?.value ?? Number.NaN)
    return Number.isFinite(parsed) ? parsed : value.getHours()
  } catch {
    return value.getHours()
  }
}
