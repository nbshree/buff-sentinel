import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

export type NormalizedRect = {
  x: number
  y: number
  width: number
  height: number
}

export type BuffAssistantActivity =
  | 'stopped'
  | 'waiting'
  | 'tracking'
  | 'prewarning'
  | 'confirming'
  | 'testing'
  | 'targetUnavailable'
  | 'error'

export type BuffTarget = {
  processName: string
  windowTitle: string
  className: string
  referenceWidth: number
  referenceHeight: number
}

export type BuffTemplateSummary = {
  id: string
  width: number
  height: number
}

export type BuffSoundCue = 'triggered' | 'prewarnThree' | 'prewarnTwo' | 'prewarnOne'

export type BuffSoundTemplateSummary = {
  id: string
  name: string
}

export type BuffCustomSoundAsset = {
  assetId: string
  fileName: string
}

export type BuffSoundSource =
  | { type: 'sine' }
  | { type: 'template'; templateId: string }
  | { type: 'custom'; assetId: string; fileName: string }

export type BuffSoundSettings = {
  triggerEnabled: boolean
  prewarnThreeEnabled: boolean
  prewarnTwoEnabled: boolean
  prewarnOneEnabled: boolean
  triggerSource: BuffSoundSource
  prewarnThreeSource: BuffSoundSource
  prewarnTwoSource: BuffSoundSource
  prewarnOneSource: BuffSoundSource
  volume: number
}

export type BuffOverlaySettings = {
  x: number
  y: number
  showWaitingDot: boolean
  excludeFromCapture: boolean
  width: number
  height: number
  colorScheme: 'gold' | 'blackWhite'
}

export type BuffCaptureSettings = {
  showSystemBorder: boolean
}

export type BuffAssistantSettings = {
  cycleMs: number
  deadlineGraceMs: number
  threshold: number
  confirmFrames: number
  missingFrames: number
  sound: BuffSoundSettings
  overlay: BuffOverlaySettings
  capture: BuffCaptureSettings
}

export type BuffAssistantConfig = {
  schemaVersion: number
  target: BuffTarget | null
  searchRegion: NormalizedRect | null
  template: BuffTemplateSummary | null
  settings: BuffAssistantSettings
}

export type BuffAssistantState = {
  config: BuffAssistantConfig
  activity: BuffAssistantActivity
  isMonitoring: boolean
  expectedAtUnixMs: number | null
  lastConfidence: number
  lastError: string | null
  captureBorderSupported: boolean
  captureBorderNotice: string | null
}

export type BorderlessCaptureAccessResult =
  'allowed' | 'unsupported' | 'deniedByUser' | 'deniedBySystem' | 'notDeclared'

export type CaptureWindowCandidate = {
  id: string
  processName: string
  windowTitle: string
  className: string
  width: number
  height: number
}

export type BuffCapturePreview = {
  dataUrl: string
  width: number
  height: number
  target: BuffTarget
}

export type BuffOverlayMode =
  | 'hidden'
  | 'waiting'
  | 'triggered'
  | 'countdown'
  | 'confirming'
  | 'reset'
  | 'targetUnavailable'
  | 'editing'

export type BuffOverlayState = {
  mode: BuffOverlayMode
  message: string
  expectedAtUnixMs: number | null
  emittedAtUnixMs: number
  editable: boolean
  colorScheme: BuffOverlaySettings['colorScheme']
}

export type BuffMetric = {
  confidence: number
  present: boolean
}

export type WindowResizeDirection =
  'East' | 'North' | 'NorthEast' | 'NorthWest' | 'South' | 'SouthEast' | 'SouthWest' | 'West'

export type WindowSize = {
  width: number
  height: number
}

export type WindowControlsAPI = {
  minimize: () => Promise<void>
  toggleMaximize: () => Promise<void>
  isMaximized: () => Promise<boolean>
  close: () => Promise<void>
  startDragging: () => Promise<void>
  startResizeDragging: (direction: WindowResizeDirection) => Promise<void>
  onResized: (callback: (size: WindowSize) => void) => () => void
}

export type BuffSentinelAPI = {
  getAppVersion: () => Promise<string>
  getBuffAssistantState: () => Promise<BuffAssistantState>
  listBuffCaptureWindows: () => Promise<CaptureWindowCandidate[]>
  listBuffSoundTemplates: () => Promise<BuffSoundTemplateSummary[]>
  captureBuffPreview: (windowId: string) => Promise<BuffCapturePreview>
  saveBuffTemplate: (
    searchRegion: NormalizedRect,
    crop: NormalizedRect,
    maskDataUrl?: string
  ) => Promise<BuffAssistantState>
  deleteBuffTemplate: () => Promise<BuffAssistantState>
  updateBuffAssistantSettings: (settings: BuffAssistantSettings) => Promise<BuffAssistantState>
  requestBuffBorderlessCaptureAccess: () => Promise<BorderlessCaptureAccessResult>
  startBuffMonitor: () => Promise<BuffAssistantState>
  stopBuffMonitor: () => Promise<BuffAssistantState>
  startBuffTemplateTest: (windowId: string) => Promise<BuffAssistantState>
  stopBuffTemplateTest: () => Promise<BuffAssistantState>
  importBuffAssistantSound: (cue: BuffSoundCue) => Promise<BuffCustomSoundAsset | null>
  playBuffAssistantSound: (
    cue: BuffSoundCue,
    source: BuffSoundSource,
    volume: number
  ) => Promise<void>
  openTtsOnline: () => Promise<void>
  setBuffOverlayEditMode: (enabled: boolean) => Promise<BuffAssistantState>
  onBuffAssistantState: (callback: (state: BuffAssistantState) => void) => () => void
  onBuffMetric: (callback: (metric: BuffMetric) => void) => () => void
  onBuffExecutionLog: (callback: (message: string) => void) => () => void
  onBuffOverlayState: (callback: (state: BuffOverlayState) => void) => () => void
  window: WindowControlsAPI
}

function callTauri<T>(operation: () => Promise<T>): Promise<T> {
  try {
    return operation()
  } catch (error) {
    return Promise.reject(error)
  }
}

function createEventListener<T>(eventName: string, callback: (payload: T) => void): () => void {
  let disposed = false
  let unlisten: UnlistenFn | undefined

  void callTauri(() =>
    listen<T>(eventName, (event) => {
      if (!disposed) callback(event.payload)
    })
  )
    .then((nextUnlisten) => {
      if (disposed) nextUnlisten()
      else unlisten = nextUnlisten
    })
    .catch((error: unknown) => {
      if (!disposed) console.error(`监听 ${eventName} 事件失败`, error)
    })

  return () => {
    disposed = true
    unlisten?.()
    unlisten = undefined
  }
}

const windowControls: WindowControlsAPI = {
  minimize: () => callTauri(() => getCurrentWindow().minimize()),
  toggleMaximize: () => callTauri(() => getCurrentWindow().toggleMaximize()),
  isMaximized: () => callTauri(() => getCurrentWindow().isMaximized()),
  close: () => callTauri(() => getCurrentWindow().close()),
  startDragging: () => callTauri(() => getCurrentWindow().startDragging()),
  startResizeDragging: (direction) =>
    callTauri(() => getCurrentWindow().startResizeDragging(direction)),
  onResized: (callback) => {
    let disposed = false
    let unlisten: UnlistenFn | undefined

    void callTauri(() =>
      getCurrentWindow().onResized(({ payload }) => {
        if (!disposed) callback({ width: payload.width, height: payload.height })
      })
    ).then((nextUnlisten) => {
      if (disposed) nextUnlisten()
      else unlisten = nextUnlisten
    })

    return () => {
      disposed = true
      unlisten?.()
      unlisten = undefined
    }
  }
}

export const buffSentinelApi: BuffSentinelAPI = {
  getAppVersion: () => callTauri(() => invoke<string>('get_app_version')),
  getBuffAssistantState: () =>
    callTauri(() => invoke<BuffAssistantState>('get_buff_assistant_state')),
  listBuffCaptureWindows: () =>
    callTauri(() => invoke<CaptureWindowCandidate[]>('list_buff_capture_windows')),
  listBuffSoundTemplates: () =>
    callTauri(() => invoke<BuffSoundTemplateSummary[]>('list_buff_sound_templates')),
  captureBuffPreview: (windowId) =>
    callTauri(() => invoke<BuffCapturePreview>('capture_buff_preview', { windowId })),
  saveBuffTemplate: (searchRegion, crop, maskDataUrl) =>
    callTauri(() =>
      invoke<BuffAssistantState>('save_buff_template', { searchRegion, crop, maskDataUrl })
    ),
  deleteBuffTemplate: () => callTauri(() => invoke<BuffAssistantState>('delete_buff_template')),
  updateBuffAssistantSettings: (settings) =>
    callTauri(() => invoke<BuffAssistantState>('update_buff_assistant_settings', { settings })),
  requestBuffBorderlessCaptureAccess: () =>
    callTauri(() =>
      invoke<BorderlessCaptureAccessResult>('request_buff_borderless_capture_access')
    ),
  startBuffMonitor: () => callTauri(() => invoke<BuffAssistantState>('start_buff_monitor')),
  stopBuffMonitor: () => callTauri(() => invoke<BuffAssistantState>('stop_buff_monitor')),
  startBuffTemplateTest: (windowId) =>
    callTauri(() => invoke<BuffAssistantState>('start_buff_template_test', { windowId })),
  stopBuffTemplateTest: () =>
    callTauri(() => invoke<BuffAssistantState>('stop_buff_template_test')),
  importBuffAssistantSound: (cue) =>
    callTauri(() => invoke<BuffCustomSoundAsset | null>('import_buff_assistant_sound', { cue })),
  playBuffAssistantSound: (cue, source, volume) =>
    callTauri(() => invoke<void>('play_buff_assistant_sound', { cue, source, volume })),
  openTtsOnline: () => callTauri(() => invoke<void>('open_tts_online')),
  setBuffOverlayEditMode: (enabled) =>
    callTauri(() => invoke<BuffAssistantState>('set_buff_overlay_edit_mode', { enabled })),
  onBuffAssistantState: (callback) => createEventListener('buff-assistant-state', callback),
  onBuffMetric: (callback) => createEventListener('buff-assistant-metric', callback),
  onBuffExecutionLog: (callback) => createEventListener('buff-assistant-execution-log', callback),
  onBuffOverlayState: (callback) => createEventListener('buff-overlay-state', callback),
  window: windowControls
}

window.api = buffSentinelApi
