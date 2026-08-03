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

    expect(loadWorkspacePreference(storage)).toBe('buffAssistant')
    expect(storage.getItem).toHaveBeenCalledWith(WORKSPACE_STORAGE_KEY)
  })

  it.each([null, 'unknown-workspace'])('falls back for invalid stored value %s', (value) => {
    expect(loadWorkspacePreference({ getItem: () => value })).toBe(DEFAULT_WORKSPACE)
  })

  it('falls back when storage cannot be read', () => {
    expect(
      loadWorkspacePreference({
        getItem: () => {
          throw new Error('storage unavailable')
        }
      })
    ).toBe(DEFAULT_WORKSPACE)
  })

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
