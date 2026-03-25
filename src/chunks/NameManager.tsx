import { useEffect, useState } from 'preact/hooks'
import { invoke } from '@tauri-apps/api/core'

import './NameManager.css'

interface Props {
  currentName: string
  onSave: (newName: string) => void
  onBack: () => void
}

type Phase = 'edit' | 'confirm'

export function NameManager({ currentName, onSave, onBack }: Props) {
  const [phase, setPhase] = useState<Phase>('edit')
  const [inputName, setInputName] = useState(currentName)
  const [knownNames, setKnownNames] = useState<string[]>([])

  useEffect(() => {
    (async () => {
      const names: string[] = await invoke('get_known_names')
      setKnownNames(names)
    })()
  }, [])

  const handleSave = () => {
    const trimmed = inputName.trim() || 'Guest'
    setInputName(trimmed)
    if (trimmed === currentName) {
      onBack()
    } else {
      setPhase('confirm')
    }
  }

  const handleMigration = async (action: 'copy' | 'move' | 'none') => {
    const newName = inputName.trim() || 'Guest'

    if (action !== 'none') {
      // Migrate file-based saves
      await invoke('migrate_saves', { action, from: currentName, to: newName })

      // Store pending localStorage migration for game launch
      const config = (await invoke('get_config')) as Config
      config.name = newName
      config.pending_migration = { action, from: currentName, to: newName }
      await invoke('write_config_file', { contents: JSON.stringify(config) })
    } else {
      const config = (await invoke('get_config')) as Config
      config.name = newName
      config.pending_migration = null
      await invoke('write_config_file', { contents: JSON.stringify(config) })
    }

    onSave(newName)
  }

  if (phase === 'confirm') {
    const newName = inputName.trim() || 'Guest'
    return (
      <div class="card name-manager">
        <div class="nm-confirm">
          <p class="nm-confirm-text">
            Do you want to move or copy the data from{' '}
            <strong>{currentName}</strong> to <strong>{newName}</strong>?
          </p>
          <div class="nm-actions">
            <button class="nm-btn nm-btn-copy" onClick={() => handleMigration('copy')}>
              Copy
            </button>
            <button class="nm-btn nm-btn-move" onClick={() => handleMigration('move')}>
              Move
            </button>
            <button class="nm-btn nm-btn-nothing" onClick={() => handleMigration('none')}>
              Do Nothing
            </button>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div class="card name-manager">
      <div class="nm-edit">
        <h2 class="nm-title">Offline Username</h2>

        <input
          class="nm-input"
          type="text"
          value={inputName}
          maxLength={16}
          onInput={(e) => setInputName((e.target as HTMLInputElement).value)}
          onKeyDown={(e) => { if (e.key === 'Enter') handleSave() }}
          autoFocus
        />

        {knownNames.length > 0 && (
          <div class="nm-known">
            <span class="nm-known-label">Saved profiles</span>
            <div class="nm-known-list">
              {knownNames.map((name) => (
                <button
                  key={name}
                  class={'nm-known-item' + (name === inputName ? ' active' : '')}
                  onClick={() => setInputName(name)}
                >
                  {name}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>

      <div class="nm-footer">
        <div class="button nm-back-btn" onClick={onBack}>
          <button>Back</button>
        </div>
        <div class="button nm-save-btn" onClick={handleSave}>
          <button>Save</button>
        </div>
      </div>
    </div>
  )
}
