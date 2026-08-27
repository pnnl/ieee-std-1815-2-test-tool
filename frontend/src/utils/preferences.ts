const STORAGE_KEY = 'mesa-profile-editor-preferences'

export type DeviceUnderTest = 'control_station' | 'outstation'

export interface Preferences {
  autoExpandSections: boolean
  controlStationIp: string
  controlStationPort: number
  outstationIp: string
  outstationPort: number
  enabledScenarios: Record<string, boolean>
  deviceUnderTest: DeviceUnderTest
}

const DEFAULT_PREFERENCES: Preferences = {
  autoExpandSections: false,
  controlStationIp: '127.0.0.1',
  controlStationPort: 20000,
  outstationIp: '127.0.0.1',
  outstationPort: 20001,
  enabledScenarios: {},
  deviceUnderTest: 'outstation',
}

export function isValidIpOrLocal(value: string): boolean {
  if (!value) return false
  if (value.toLowerCase() === 'local') return true

  const ipv4Regex = /^(\d{1,3}\.){3}\d{1,3}$/
  if (!ipv4Regex.test(value)) return false

  const octets = value.split('.')
  return octets.every((octet) => {
    const num = parseInt(octet, 10)
    return num >= 0 && num <= 255
  })
}

export function isValidPort(value: number | string): boolean {
  const port = typeof value === 'string' ? parseInt(value, 10) : value
  return !isNaN(port) && port >= 1 && port <= 65535
}

export function loadPreferences(): Preferences {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored) {
      const parsed = JSON.parse(stored) as Preferences
      return { ...DEFAULT_PREFERENCES, ...parsed }
    }
  } catch (error) {
    console.error('Failed to load preferences:', error)
  }
  return { ...DEFAULT_PREFERENCES }
}

export function savePreferences(preferences: Preferences): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences))
  } catch (error) {
    console.error('Failed to save preferences:', error)
  }
}

export function updatePreference(
  currentPreferences: Preferences,
  key: keyof Preferences,
  value: Preferences[keyof Preferences],
): Preferences {
  const updated = { ...currentPreferences, [key]: value }
  savePreferences(updated)
  return updated
}
