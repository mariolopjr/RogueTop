import { registerMenuKeybind } from './keys'
import { createNotifSection, showNotif } from './notifs'
import { startRPCTracker } from './rpc'
import { waitForElement } from './utils'

// True when the game is being served from the RogueTop offline server (port 7653)
// Online mode loads from pokerogue.net and must NOT have its API calls intercepted
const isOffline = window.location.port === '7653'

async function init() {
  console.log('RogueTop injected successfully!')

  proxyFetch()

  // If we are not actually in pokerogue, but in the main menu, don't do anything
  if (await waitForElement('#root', 1000).catch(() => null) !== null) {
    console.log('Not in pokerogue, don\'t mind me!')
    return
  }

  // In offline mode, set a fake session cookie so the game's hasSession check
  // passes and updateUserInfo() calls our /account/info endpoint (returns Guest)
  if (isOffline) {
    document.cookie = 'pokerogue_sessionId=offline; path=/'

    // Execute any pending localStorage key migration (from a username rename)
    // @ts-expect-error womp womp
    const config: Config = await __TAURI_INTERNALS__.invoke('get_config')
    if (config.pending_migration) {
      const { action, from, to } = config.pending_migration
      const KEYS = [
        'data', 'sessionData', 'sessionData1', 'sessionData2',
        'sessionData3', 'sessionData4', 'runHistoryData', 'starterPrefs',
      ]
      for (const key of KEYS) {
        const oldVal = localStorage.getItem(`${key}_${from}`)
        if (oldVal !== null) {
          localStorage.setItem(`${key}_${to}`, oldVal)
          if (action === 'move') localStorage.removeItem(`${key}_${from}`)
        }
      }
      config.pending_migration = null
      // @ts-expect-error womp womp
      await __TAURI_INTERNALS__.invoke('write_config_file', {
        contents: JSON.stringify(config),
      })
    }
  }

  // Register binds
  registerMenuKeybind()

  console.log('Fetch proxied successfully!')

  // load user plugins
  console.log('Loading user plugins...')
  // @ts-expect-error womp womp
  await __TAURI_INTERNALS__.invoke('load_all_plugins')

  // Inject the notification section
  if (document.querySelector('.notif-section') === null) {
    createNotifSection()
  }

  console.log('Notif section created successfully!')

  await waitForElement('.notif-section')

  showNotif('Press F1 to return to the RogueTop menu', 3000)

  startRPCTracker()
}

function proxyFetch() {
  // overwrite fetch to send to the Tauri backend
  // @ts-expect-error womp womp
  window.nativeFetch = window.fetch

  // @ts-expect-error womp womp
  window.fetch = async (url: string, options: RequestInit) => {
    // Offline: intercept both localhost:8001 (normal path) and api.pokerogue.net
    // Online: only intercept :8001 — api.pokerogue.net must pass through directly
    const shouldProxy = isOffline
      ? (url.includes(':8001') || url.includes('api.pokerogue.net'))
      : url.includes(':8001')

    if (!shouldProxy) {
      // Forward to regular fetch
      // @ts-expect-error womp womp
      return window.nativeFetch(url, options)
    }

    // @ts-expect-error womp womp
    const response: { status: number, body: string } = await __TAURI_INTERNALS__.invoke('api_request', {
      url,
      options: JSON.stringify(options ?? {})
    })

    // Adherence to what most scripts will expect to have available when they are using fetch(). These have to pretend to be promises
    return {
      json: async () => JSON.parse(response.body),
      text: async () => response.body,
      arrayBuffer: async () => new TextEncoder().encode(response.body).buffer,
      ok: response.status >= 200 && response.status < 300,
      status: response.status,
    }
  }
}

init()
