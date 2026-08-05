import { Channel, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

export type MacroPointAction = 'click' | 'doubleClick' | 'key'

export type MacroPoint = {
  id: string
  label: string
  action: MacroPointAction
  enabled: boolean
  x: number
  y: number
  key: string
  modifiers: Array<'Control' | 'Alt' | 'Shift'>
  delaySeconds: number
  createdAt: number
}

export type MacroPointPatch = Partial<
  Pick<
    MacroPoint,
    'label' | 'action' | 'enabled' | 'x' | 'y' | 'key' | 'modifiers' | 'delaySeconds'
  >
>

export type MacroSettings = {
  clickIntervalSeconds: number
  loopIntervalSeconds: number
  startDelaySeconds: number
  loopMode: 'count' | 'infinite'
  loopCount: number
  hotkeys: {
    capture: string
    start: string
    stop: string
  }
}

export type AppearancePreferences = {
  themeId: string
  cleanMode: boolean
}

export type MysteryCodeStatus = {
  configured: boolean
  lastFour: string | null
  baseUrl: string
  apiKeyConfigured?: boolean
  apiKeyLastFour?: string | null
}

export type InternalSkillRecognitionResult = {
  baseStats: {
    season: number
    strengthOrQi: number
    attack: number
    armorPenetration: number
    factionRestraint: number
    criticalHit: number
    maxAttack: number
    minAttack: number
    agility: number
    endurance: number
    constitution: number
  }
  equippedSkillIds: string[]
}

export type AppUpdateInfo = {
  version: string
  notes: string
  publishedAt: string | null
}

export type AppUpdateCheckResult = {
  currentVersion: string
  update: AppUpdateInfo | null
}

export type AppUpdateDownloadEvent = {
  event: 'started' | 'progress' | 'finished'
  downloaded: number
  total: number | null
}

export type MacroState = {
  points: MacroPoint[]
  settings: MacroSettings
  appearance: AppearancePreferences
  activeProfileId: string
  profiles: Array<{
    id: string
    name: string
    updatedAt: number
  }>
  isRecording: boolean
  isRunning: boolean
  currentIndex: number
  countdownRemaining: number
  completedLoops: number
  hotkeyErrors: string[]
  logs: string[]
}

export type GameRecorderActivity =
  'idle' | 'recordingCountdown' | 'recording' | 'playbackCountdown' | 'playing'

export type GameRecorderHotkeys = {
  recordStart: string
  stop: string
  playbackStart: string
}

export type GamePlaybackSettings = {
  speed: 0.5 | 1 | 1.5 | 2
  loopMode: 'count' | 'infinite'
  loopCount: number
  loopIntervalSeconds: number
}

export type GameRecordingTarget = {
  processName: string
  windowTitle: string
}

export type GameRecordedEvent =
  | { atMs: number; type: 'mouseMove'; dx: number; dy: number }
  | {
      atMs: number
      type: 'mouseButton'
      button: 'left' | 'right' | 'middle'
      pressed: boolean
    }
  | { atMs: number; type: 'mouseWheel'; delta: number }
  | { atMs: number; type: 'key'; scanCode: number; extended: boolean; pressed: boolean }

export type GameRecordingSummary = {
  id: string
  name: string
  durationMs: number
  eventCount: number
  keyboardEventCount: number
  mouseEventCount: number
  target: GameRecordingTarget
  createdAt: number
  updatedAt: number
  playback: GamePlaybackSettings
}

export type GameRecorderState = {
  recordings: GameRecordingSummary[]
  activeRecordingId: string | null
  hotkeys: GameRecorderHotkeys
  activity: GameRecorderActivity
  countdownRemaining: number
  completedLoops: number
  targetMismatch: boolean
  hotkeyErrors: string[]
  lastError: string | null
}

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
  width: number
  height: number
  showBorder: boolean
  colorScheme: 'gold' | 'blackWhite'
}

export type BuffAssistantSettings = {
  cycleMs: number
  deadlineGraceMs: number
  threshold: number
  confirmFrames: number
  missingFrames: number
  sound: BuffSoundSettings
  overlay: BuffOverlaySettings
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
}

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
  showBorder: boolean
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

export type Workspace =
  'macro' | 'gameRecorder' | 'buffAssistant' | 'calculator' | 'towerCalculator'

export type WindowControlsAPI = {
  minimize: () => Promise<void>
  toggleMaximize: () => Promise<void>
  isMaximized: () => Promise<boolean>
  close: () => Promise<void>
  startDragging: () => Promise<void>
  startResizeDragging: (direction: WindowResizeDirection) => Promise<void>
  onResized: (callback: (size: WindowSize) => void) => () => void
}

export type MacroAPI = {
  getAppVersion: () => Promise<string>
  switchWorkspace: (workspace: Workspace) => Promise<void>
  getState: () => Promise<MacroState>
  startRecording: () => Promise<MacroState>
  stopRecording: () => Promise<MacroState>
  startRun: () => Promise<MacroState>
  stopRun: () => Promise<MacroState>
  clearLogs: () => Promise<MacroState>
  removePoint: (id: string) => Promise<MacroState>
  clearPoints: () => Promise<MacroState>
  addKeyPoint: (key: string, modifiers: MacroPoint['modifiers']) => Promise<MacroState>
  setKeyCapture: (enabled: boolean) => Promise<void>
  syncPointDelays: () => Promise<MacroState>
  updatePoint: (id: string, patch: MacroPointPatch) => Promise<MacroState>
  movePoint: (id: string, direction: 'up' | 'down') => Promise<MacroState>
  reorderPoint: (id: string, targetIndex: number) => Promise<MacroState>
  testPoint: (id: string) => Promise<MacroState>
  updateSettings: (settings: Partial<MacroSettings>) => Promise<MacroState>
  updateAppearance: (appearance: Partial<AppearancePreferences>) => Promise<MacroState>
  createProfile: (name: string) => Promise<MacroState>
  switchProfile: (id: string) => Promise<MacroState>
  renameProfile: (id: string, name: string) => Promise<MacroState>
  deleteProfile: (id: string) => Promise<MacroState>
  exportProfile: (id: string) => Promise<MacroState>
  importProfile: () => Promise<MacroState>
  getMysteryCodeStatus: () => Promise<MysteryCodeStatus>
  openAiProviderRegistration: () => Promise<void>
  saveAndValidateMysteryCode: (
    mysteryCode: string,
    baseUrl: string,
    apiKey: string
  ) => Promise<MysteryCodeStatus>
  deleteMysteryCode: () => Promise<MysteryCodeStatus>
  recognizeInternalSkillImage: (imageDataUrl: string) => Promise<InternalSkillRecognitionResult>
  checkForUpdate: () => Promise<AppUpdateCheckResult>
  installUpdate: (onEvent: (event: AppUpdateDownloadEvent) => void) => Promise<void>
  onState: (callback: (state: MacroState) => void) => () => void
  getGameRecorderState: () => Promise<GameRecorderState>
  startGameRecording: () => Promise<GameRecorderState>
  stopGameActivity: () => Promise<GameRecorderState>
  startGamePlayback: (allowTargetMismatch?: boolean) => Promise<GameRecorderState>
  selectGameRecording: (id: string) => Promise<GameRecorderState>
  renameGameRecording: (id: string, name: string) => Promise<GameRecorderState>
  deleteGameRecording: (id: string) => Promise<GameRecorderState>
  updateGameRecorderHotkeys: (hotkeys: GameRecorderHotkeys) => Promise<GameRecorderState>
  updateGamePlaybackSettings: (
    id: string,
    settings: GamePlaybackSettings
  ) => Promise<GameRecorderState>
  onGameRecorderState: (callback: (state: GameRecorderState) => void) => () => void
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

type StateCommand =
  | 'get_state'
  | 'start_recording'
  | 'stop_recording'
  | 'start_run'
  | 'stop_run'
  | 'clear_logs'
  | 'remove_point'
  | 'clear_points'
  | 'add_key_point'
  | 'sync_point_delays'
  | 'update_point'
  | 'move_point'
  | 'reorder_point'
  | 'test_point'
  | 'update_settings'
  | 'update_appearance'
  | 'create_profile'
  | 'switch_profile'
  | 'rename_profile'
  | 'delete_profile'
  | 'export_profile'
  | 'import_profile'

function invokeState(command: StateCommand, args?: Record<string, unknown>): Promise<MacroState> {
  return callTauri(() => invoke<MacroState>(command, args))
}

type GameRecorderStateCommand =
  | 'get_game_recorder_state'
  | 'start_game_recording'
  | 'stop_game_activity'
  | 'start_game_playback'
  | 'select_game_recording'
  | 'rename_game_recording'
  | 'delete_game_recording'
  | 'update_game_recorder_hotkeys'
  | 'update_game_playback_settings'

function invokeGameRecorderState(
  command: GameRecorderStateCommand,
  args?: Record<string, unknown>
): Promise<GameRecorderState> {
  return callTauri(() => invoke<GameRecorderState>(command, args))
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
    )
      .then((nextUnlisten) => {
        if (disposed) nextUnlisten()
        else unlisten = nextUnlisten
      })
      .catch((error: unknown) => {
        if (!disposed) console.error('监听窗口尺寸变化失败', error)
      })

    return () => {
      disposed = true
      unlisten?.()
      unlisten = undefined
    }
  }
}

export const macroApi: MacroAPI = {
  getAppVersion: () => callTauri(() => invoke<string>('get_app_version')),
  switchWorkspace: (workspace) => callTauri(() => invoke<void>('switch_workspace', { workspace })),
  getState: () => invokeState('get_state'),
  startRecording: () => invokeState('start_recording'),
  stopRecording: () => invokeState('stop_recording'),
  startRun: () => invokeState('start_run'),
  stopRun: () => invokeState('stop_run'),
  clearLogs: () => invokeState('clear_logs'),
  removePoint: (id) => invokeState('remove_point', { id }),
  clearPoints: () => invokeState('clear_points'),
  addKeyPoint: (key, modifiers) => invokeState('add_key_point', { key, modifiers }),
  setKeyCapture: (enabled) => callTauri(() => invoke<void>('set_key_capture', { enabled })),
  syncPointDelays: () => invokeState('sync_point_delays'),
  updatePoint: (id, patch) => invokeState('update_point', { id, patch }),
  movePoint: (id, direction) => invokeState('move_point', { id, direction }),
  reorderPoint: (id, targetIndex) => invokeState('reorder_point', { id, targetIndex }),
  testPoint: (id) => invokeState('test_point', { id }),
  updateSettings: (settings) => invokeState('update_settings', { settings }),
  updateAppearance: (appearance) => invokeState('update_appearance', { appearance }),
  createProfile: (name) => invokeState('create_profile', { name }),
  switchProfile: (id) => invokeState('switch_profile', { id }),
  renameProfile: (id, name) => invokeState('rename_profile', { id, name }),
  deleteProfile: (id) => invokeState('delete_profile', { id }),
  exportProfile: (id) => invokeState('export_profile', { id }),
  importProfile: () => invokeState('import_profile'),
  getMysteryCodeStatus: () => callTauri(() => invoke<MysteryCodeStatus>('get_mystery_code_status')),
  openAiProviderRegistration: () => callTauri(() => invoke<void>('open_ai_provider_registration')),
  saveAndValidateMysteryCode: (mysteryCode, baseUrl, apiKey) =>
    callTauri(() =>
      invoke<MysteryCodeStatus>('save_and_validate_mystery_code', {
        mysteryCode,
        baseUrl,
        apiKey
      })
    ),
  deleteMysteryCode: () => callTauri(() => invoke<MysteryCodeStatus>('delete_mystery_code')),
  recognizeInternalSkillImage: (imageDataUrl) =>
    callTauri(() =>
      invoke<InternalSkillRecognitionResult>('recognize_internal_skill_image', { imageDataUrl })
    ),
  checkForUpdate: () => callTauri(() => invoke<AppUpdateCheckResult>('check_for_update')),
  installUpdate: (onEvent) => {
    const eventChannel = new Channel<AppUpdateDownloadEvent>()
    eventChannel.onmessage = onEvent
    return callTauri(() => invoke<void>('install_update', { onEvent: eventChannel }))
  },
  onState: (callback) => {
    let disposed = false
    let unlisten: UnlistenFn | undefined

    void callTauri(() =>
      listen<MacroState>('macro-state', (event) => {
        if (!disposed) callback(event.payload)
      })
    )
      .then((nextUnlisten) => {
        if (disposed) nextUnlisten()
        else unlisten = nextUnlisten
      })
      .catch((error: unknown) => {
        if (!disposed) console.error('监听 macro-state 事件失败', error)
      })

    return () => {
      disposed = true
      unlisten?.()
      unlisten = undefined
    }
  },
  getGameRecorderState: () => invokeGameRecorderState('get_game_recorder_state'),
  startGameRecording: () => invokeGameRecorderState('start_game_recording'),
  stopGameActivity: () => invokeGameRecorderState('stop_game_activity'),
  startGamePlayback: (allowTargetMismatch = false) =>
    invokeGameRecorderState('start_game_playback', { allowTargetMismatch }),
  selectGameRecording: (id) => invokeGameRecorderState('select_game_recording', { id }),
  renameGameRecording: (id, name) => invokeGameRecorderState('rename_game_recording', { id, name }),
  deleteGameRecording: (id) => invokeGameRecorderState('delete_game_recording', { id }),
  updateGameRecorderHotkeys: (hotkeys) =>
    invokeGameRecorderState('update_game_recorder_hotkeys', { hotkeys }),
  updateGamePlaybackSettings: (id, settings) =>
    invokeGameRecorderState('update_game_playback_settings', { id, settings }),
  onGameRecorderState: (callback) => {
    let disposed = false
    let unlisten: UnlistenFn | undefined

    void callTauri(() =>
      listen<GameRecorderState>('game-recorder-state', (event) => {
        if (!disposed) callback(event.payload)
      })
    )
      .then((nextUnlisten) => {
        if (disposed) nextUnlisten()
        else unlisten = nextUnlisten
      })
      .catch((error: unknown) => {
        if (!disposed) console.error('监听 game-recorder-state 事件失败', error)
      })

    return () => {
      disposed = true
      unlisten?.()
      unlisten = undefined
    }
  },
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

window.api = macroApi
