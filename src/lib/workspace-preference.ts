import type { TrayWorkspace } from './macro-api'

export type WorkspaceView = TrayWorkspace

export const WORKSPACE_ORDER = [
  'macro',
  'buffAssistant',
  'gameRecorder',
  'calculator',
  'towerCalculator'
] as const satisfies readonly WorkspaceView[]

export const DEFAULT_WORKSPACE: WorkspaceView = 'macro'
export const WORKSPACE_STORAGE_KEY = 'shree-macro-flow.active-workspace'

type WorkspaceStorageReader = Pick<Storage, 'getItem'>
type WorkspaceStorageWriter = Pick<Storage, 'setItem'>

export function isWorkspaceView(value: unknown): value is WorkspaceView {
  return typeof value === 'string' && WORKSPACE_ORDER.includes(value as WorkspaceView)
}

export function loadWorkspacePreference(storage?: WorkspaceStorageReader): WorkspaceView {
  try {
    const storedWorkspace = (storage ?? window.localStorage).getItem(WORKSPACE_STORAGE_KEY)
    return isWorkspaceView(storedWorkspace) ? storedWorkspace : DEFAULT_WORKSPACE
  } catch {
    return DEFAULT_WORKSPACE
  }
}

export function saveWorkspacePreference(
  workspace: WorkspaceView,
  storage?: WorkspaceStorageWriter
): void {
  try {
    const storageTarget = storage ?? window.localStorage
    storageTarget.setItem(WORKSPACE_STORAGE_KEY, workspace)
  } catch {
    // Workspace selection remains usable when WebView storage is unavailable.
  }
}
