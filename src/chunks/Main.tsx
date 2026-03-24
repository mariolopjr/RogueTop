import { useEffect, useState } from 'preact/hooks'
import { invoke } from '@tauri-apps/api/core'
import { Checkbox } from '../components/Checkbox'
import { NameManager } from './NameManager'

import './Main.css'

export function Main() {
  const [selected, setSelected] = useState('online')
  const [alwaysUse, setAlwaysUse] = useState(false)
  const [rpc, setRPC] = useState(true)
  const [supportsOffline, setSupportsOffline] = useState(false)
  const [offlineName, setOfflineName] = useState('Guest')
  const [view, setView] = useState<'main' | 'name-manager'>('main')

  // Load the config
  useEffect(() => {
    (async () => {
      const config: Config = await invoke('get_config')
      const supportsOffline: boolean = await invoke('supports_offline')
      setSupportsOffline(supportsOffline)
      setAlwaysUse(config.skip_splash)
      setRPC(config.rpc)
      setSelected(config.offline ? 'offline' : 'online')
      setOfflineName(config.name ?? 'Guest')
    })()
  }, [])

  const setConfig = async (
    option: keyof Config,
    value: Config[keyof Config]
  ) => {
    const config = (await invoke('get_config')) as Config
    config[option] = value
    await invoke('write_config_file', {
      contents: JSON.stringify(config),
    })
  }

  if (view === 'name-manager') {
    return (
      <NameManager
        currentName={offlineName}
        onSave={(newName) => {
          setOfflineName(newName)
          setView('main')
        }}
        onBack={() => setView('main')}
      />
    )
  }

  return (
    <div class="card">
      <div class="mode-select">
        <div
          class={'mode ' + (selected === 'online' ? 'selected' : '')}
          id="online"
          onClick={() => {
            setSelected('online')
            setConfig('offline', false)
          }}
        >
          <span class="mode-title">Online (RECOMMENDED)</span>

          <div class="mode-img">
            <img src="arrow.svg" alt="Offline" />
          </div>
        </div>

        {supportsOffline && (
          <div
            class={'mode ' + (selected === 'offline' ? 'selected' : '')}
            id="offline"
            onClick={() => {
              setSelected('offline')
              setConfig('offline', true)
            }}
          >
            <span class="mode-title">Offline (LOCAL)</span>

            <div class="mode-img">
              <img src="arrow.svg" alt="Offline" />
            </div>
          </div>
        )}

        <div class="cbx-setting">
          <Checkbox
            initialChecked={alwaysUse}
            id="always-use"
            label="Always use this mode"
            onChange={() => {
              setAlwaysUse(!alwaysUse)
              setConfig('skip_splash', !alwaysUse)
            }}
          />
        </div>

        <div class="cbx-setting">
          <Checkbox
            initialChecked={rpc}
            id="rpc-enable"
            label={<>Enable Discord RPC<br />(requires restart)</>}
            onChange={() => {
              setRPC(!rpc)
              setConfig('rpc', !rpc)
            }}
          />
        </div>

        {selected === 'offline' && (
          <div class="name-setting">
            <button class="name-btn" onClick={() => setView('name-manager')}>
              Username: {offlineName} ▸
            </button>
          </div>
        )}
      </div>
      <div
        id="play"
        class="button"
        onClick={() => {
          // If the build doesn't support offline but offline is selected, don't launch
          if (!supportsOffline && selected === 'offline') {
            return
          }

          invoke('launch')
        }}
      >
        <button>Play</button>
      </div>
    </div>
  )
}
