import { describe, expect, it, vi } from 'vitest'

import {
  DEFAULT_WORKSPACE,
  loadWorkspacePreference,
  saveWorkspacePreference,
  WORKSPACE_STORAGE_KEY
} from './workspace-preference'

describe('workspace preference', () => {
  it('restores a supported workspace', () => {
    const storage = { getItem: vi.fn(() => 'buffAssistant') }

    expect(loadWorkspacePreference(false, storage)).toBe('buffAssistant')
    expect(storage.getItem).toHaveBeenCalledWith(WORKSPACE_STORAGE_KEY)
  })

  it.each([null, 'unknown-workspace'])('falls back for invalid stored value %s', (value) => {
    expect(loadWorkspacePreference(true, { getItem: () => value })).toBe(DEFAULT_WORKSPACE)
  })

  it.each(['calculator', 'towerCalculator'])(
    'falls back when the stored workspace %s is hidden',
    (value) => {
      expect(loadWorkspacePreference(true, { getItem: () => value })).toBe(DEFAULT_WORKSPACE)
    }
  )

  it('falls back when storage cannot be read', () => {
    expect(
      loadWorkspacePreference(false, {
        getItem: () => {
          throw new Error('storage unavailable')
        }
      })
    ).toBe(DEFAULT_WORKSPACE)
  })

  it.each(['macro', 'gameRecorder'])(
    'blocks stored workspace %s while access is locked',
    (value) => {
      expect(loadWorkspacePreference(false, { getItem: () => value })).toBe(DEFAULT_WORKSPACE)
    }
  )

  it.each(['macro', 'gameRecorder'])(
    'restores stored workspace %s after access is unlocked',
    (value) => {
      expect(loadWorkspacePreference(true, { getItem: () => value })).toBe(value)
    }
  )

  it('persists the selected workspace', () => {
    const storage = { setItem: vi.fn() }

    saveWorkspacePreference('towerCalculator', storage)

    expect(storage.setItem).toHaveBeenCalledWith(WORKSPACE_STORAGE_KEY, 'towerCalculator')
  })

  it('does not throw when storage cannot be written', () => {
    expect(() =>
      saveWorkspacePreference('macro', {
        setItem: () => {
          throw new Error('storage unavailable')
        }
      })
    ).not.toThrow()
  })
})
