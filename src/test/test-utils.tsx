import { render, type RenderOptions } from '@testing-library/react'
import type { ReactElement, ReactNode } from 'react'
import { vi } from 'vitest'

import { TooltipProvider } from '@/components/ui/tooltip'
import {
  emptyGameRecorderState,
  type GameRecorderController
} from '@/hooks/useGameRecorderController'
import { emptyState, type MacroController } from '@/hooks/useMacroController'
import type {
  BuffAssistantState,
  GameRecorderState,
  MacroAPI,
  MacroPointPatch,
  MacroState
} from '@/lib/macro-api'

export function createMacroState(overrides: Partial<MacroState> = {}): MacroState {
  const state: MacroState = {
    ...emptyState,
    points: emptyState.points.map((point) => ({ ...point, modifiers: [...point.modifiers] })),
    settings: {
      ...emptyState.settings,
      hotkeys: { ...emptyState.settings.hotkeys }
    },
    appearance: { ...emptyState.appearance },
    profiles: emptyState.profiles.map((profile) => ({ ...profile })),
    hotkeyErrors: [...emptyState.hotkeyErrors],
    logs: [...emptyState.logs]
  }

  return {
    ...state,
    ...overrides,
    settings: overrides.settings
      ? {
          ...state.settings,
          ...overrides.settings,
          hotkeys: { ...state.settings.hotkeys, ...overrides.settings.hotkeys }
        }
      : state.settings,
    appearance: { ...state.appearance, ...overrides.appearance },
    points: overrides.points ?? state.points,
    profiles: overrides.profiles ?? state.profiles,
    hotkeyErrors: overrides.hotkeyErrors ?? state.hotkeyErrors,
    logs: overrides.logs ?? state.logs
  }
}

export function createMacroController(overrides: Partial<MacroController> = {}): MacroController {
  const state = overrides.state ?? createMacroState()
  const draftSettings = overrides.draftSettings ?? {
    ...state.settings,
    hotkeys: { ...state.settings.hotkeys }
  }
  const enabledPointCount =
    overrides.enabledPointCount ?? state.points.filter((point) => point.enabled).length

  const controller: MacroController = {
    state,
    draftSettings,
    setDraftSettings: vi.fn<MacroController['setDraftSettings']>(),
    draftPoints: {},
    profileNameInput: '',
    setProfileNameInput: vi.fn<MacroController['setProfileNameInput']>(),
    isRenamingProfile: false,
    setIsRenamingProfile: vi.fn<MacroController['setIsRenamingProfile']>(),
    capturingHotkey: null,
    draggingPointId: null,
    setDraggingPointId: vi.fn<MacroController['setDraggingPointId']>(),
    isAddingKeyStep: false,
    capturingPointKeyId: null,
    setCapturingPointKeyId: vi.fn<MacroController['setCapturingPointKeyId']>(),
    keyDraft: { key: '', modifiers: [] },
    keyStepError: '',
    logPanelHeight: 140,
    logPanelMaxHeight: 420,
    profileNameInputRef: { current: null },
    isEditingLocked: false,
    hasUnsavedChanges: false,
    canStopRecording: false,
    enabledPointCount,
    status: { label: '待命', tone: 'muted' },
    targetLoops: state.settings.loopMode === 'infinite' ? '无限' : String(state.settings.loopCount),
    updateState: vi.fn<MacroController['updateState']>().mockResolvedValue(undefined),
    updateAppearance: vi.fn<MacroController['updateAppearance']>().mockResolvedValue(undefined),
    updateDraftNumber: vi.fn<MacroController['updateDraftNumber']>(),
    startHotkeyCapture: vi.fn<MacroController['startHotkeyCapture']>(),
    stopHotkeyCapture: vi.fn<MacroController['stopHotkeyCapture']>(),
    captureHotkey: vi.fn<MacroController['captureHotkey']>(),
    updateDraftPoint: vi.fn<MacroController['updateDraftPoint']>(),
    updatePoint: vi.fn<MacroController['updatePoint']>().mockResolvedValue(undefined),
    savePoint: vi.fn<MacroController['savePoint']>(),
    syncDefaultDelayToPoints: vi
      .fn<MacroController['syncDefaultDelayToPoints']>()
      .mockResolvedValue(undefined),
    closeKeyStepEditor: vi.fn<MacroController['closeKeyStepEditor']>(),
    openKeyStepEditor: vi.fn<MacroController['openKeyStepEditor']>(),
    captureKeyStep: vi.fn<MacroController['captureKeyStep']>(),
    saveKeyStep: vi.fn<MacroController['saveKeyStep']>(),
    capturePointKey: vi.fn<MacroController['capturePointKey']>(),
    dropPoint: vi.fn<MacroController['dropPoint']>(),
    startResizeLogPanel: vi.fn<MacroController['startResizeLogPanel']>(),
    resizeLogPanelBy: vi.fn<MacroController['resizeLogPanelBy']>(),
    createProfile: vi.fn<MacroController['createProfile']>(),
    renameActiveProfile: vi.fn<MacroController['renameActiveProfile']>(),
    cancelRenameProfile: vi.fn<MacroController['cancelRenameProfile']>(),
    removeActiveProfile: vi.fn<MacroController['removeActiveProfile']>()
  }

  return { ...controller, ...overrides, state, draftSettings, enabledPointCount }
}

export function createGameRecorderState(
  overrides: Partial<GameRecorderState> = {}
): GameRecorderState {
  return {
    ...emptyGameRecorderState,
    ...overrides,
    recordings: overrides.recordings ?? [],
    hotkeys: { ...emptyGameRecorderState.hotkeys, ...overrides.hotkeys },
    hotkeyErrors: overrides.hotkeyErrors ?? []
  }
}

export function createGameRecorderController(
  overrides: Partial<GameRecorderController> = {}
): GameRecorderController {
  const state = overrides.state ?? createGameRecorderState()
  const selectedRecording =
    overrides.selectedRecording ??
    state.recordings.find((recording) => recording.id === state.activeRecordingId) ??
    null
  const draftHotkeys = overrides.draftHotkeys ?? { ...state.hotkeys }
  const draftPlayback = overrides.draftPlayback ??
    selectedRecording?.playback ?? {
      speed: 1,
      loopMode: 'count',
      loopCount: 1,
      loopIntervalSeconds: 1
    }

  const controller: GameRecorderController = {
    state,
    selectedRecording,
    draftHotkeys,
    draftPlayback,
    setDraftPlayback: vi.fn<GameRecorderController['setDraftPlayback']>(),
    nameInput: selectedRecording?.name ?? '',
    setNameInput: vi.fn<GameRecorderController['setNameInput']>(),
    capturingHotkey: null,
    pendingAction: null,
    actionError: null,
    blockedByMacro: false,
    isBusy: state.activity !== 'idle',
    isIdle: state.activity === 'idle',
    hasHotkeyChanges: false,
    hasPlaybackChanges: false,
    hasNameChanges: false,
    targetMismatchPromptOpen: false,
    status: { label: '待命', tone: 'muted' },
    progressLabel: '尚未开始',
    startRecording: vi.fn<GameRecorderController['startRecording']>().mockResolvedValue(undefined),
    stopActivity: vi.fn<GameRecorderController['stopActivity']>().mockResolvedValue(undefined),
    startPlayback: vi.fn<GameRecorderController['startPlayback']>().mockResolvedValue(undefined),
    selectRecording: vi
      .fn<GameRecorderController['selectRecording']>()
      .mockResolvedValue(undefined),
    renameSelected: vi.fn<GameRecorderController['renameSelected']>().mockResolvedValue(undefined),
    deleteSelected: vi.fn<GameRecorderController['deleteSelected']>().mockResolvedValue(undefined),
    saveHotkeys: vi.fn<GameRecorderController['saveHotkeys']>().mockResolvedValue(undefined),
    savePlayback: vi.fn<GameRecorderController['savePlayback']>().mockResolvedValue(undefined),
    setPlaybackSpeed: vi.fn<GameRecorderController['setPlaybackSpeed']>(),
    setPlaybackLoopMode: vi.fn<GameRecorderController['setPlaybackLoopMode']>(),
    updatePlaybackNumber: vi.fn<GameRecorderController['updatePlaybackNumber']>(),
    startHotkeyCapture: vi.fn<GameRecorderController['startHotkeyCapture']>(),
    stopHotkeyCapture: vi.fn<GameRecorderController['stopHotkeyCapture']>(),
    captureHotkey: vi.fn<GameRecorderController['captureHotkey']>(),
    dismissTargetMismatch: vi.fn<GameRecorderController['dismissTargetMismatch']>()
  }

  return {
    ...controller,
    ...overrides,
    state,
    selectedRecording,
    draftHotkeys,
    draftPlayback
  }
}

export function createMacroApi(
  state: MacroState = createMacroState(),
  gameRecorderState: GameRecorderState = createGameRecorderState()
) {
  const buffState: BuffAssistantState = {
    config: {
      schemaVersion: 5,
      target: null,
      searchRegion: null,
      template: null,
      settings: {
        cycleMs: 20_000,
        deadlineGraceMs: 600,
        threshold: 0.95,
        confirmFrames: 3,
        missingFrames: 5,
        sound: {
          triggerEnabled: true,
          prewarnThreeEnabled: true,
          prewarnTwoEnabled: true,
          prewarnOneEnabled: true,
          triggerSource: { type: 'sine' },
          prewarnThreeSource: { type: 'sine' },
          prewarnTwoSource: { type: 'sine' },
          prewarnOneSource: { type: 'sine' },
          volume: 0.45
        },
        overlay: {
          x: 40,
          y: 100,
          showWaitingDot: false,
          width: 330,
          height: 92,
          showBorder: true,
          colorScheme: 'gold'
        }
      }
    },
    activity: 'stopped',
    isMonitoring: false,
    expectedAtUnixMs: null,
    lastConfidence: 0,
    lastError: null
  }
  return {
    getAppVersion: vi.fn<MacroAPI['getAppVersion']>(async () => '1.8.1'),
    switchWorkspace: vi.fn<MacroAPI['switchWorkspace']>(async () => undefined),
    getState: vi.fn(async () => state),
    startRecording: vi.fn(async () => state),
    stopRecording: vi.fn(async () => state),
    startRun: vi.fn(async () => state),
    stopRun: vi.fn(async () => state),
    clearLogs: vi.fn(async () => state),
    removePoint: vi.fn(async (_id: string) => state),
    clearPoints: vi.fn(async () => state),
    addKeyPoint: vi.fn(
      async (_key: string, _modifiers: MacroState['points'][number]['modifiers']) => state
    ),
    setKeyCapture: vi.fn(async (_enabled: boolean) => undefined),
    syncPointDelays: vi.fn(async () => state),
    updatePoint: vi.fn(async (_id: string, _patch: MacroPointPatch) => state),
    movePoint: vi.fn(async (_id: string, _direction: 'up' | 'down') => state),
    reorderPoint: vi.fn(async (_id: string, _targetIndex: number) => state),
    testPoint: vi.fn(async (_id: string) => state),
    updateSettings: vi.fn(async (_settings: Partial<MacroState['settings']>) => state),
    updateAppearance: vi.fn(async (_appearance: Partial<MacroState['appearance']>) => state),
    createProfile: vi.fn(async (_name: string) => state),
    switchProfile: vi.fn(async (_id: string) => state),
    renameProfile: vi.fn(async (_id: string, _name: string) => state),
    deleteProfile: vi.fn(async (_id: string) => state),
    exportProfile: vi.fn(async (_id: string) => state),
    importProfile: vi.fn(async () => state),
    getMysteryCodeStatus: vi.fn<MacroAPI['getMysteryCodeStatus']>(async () => ({
      configured: false,
      lastFour: null,
      baseUrl: 'https://gzxsy.vip',
      apiKeyConfigured: false,
      apiKeyLastFour: null
    })),
    openAiProviderRegistration: vi.fn<MacroAPI['openAiProviderRegistration']>(async () => {}),
    saveAndValidateMysteryCode: vi.fn<MacroAPI['saveAndValidateMysteryCode']>(async () => ({
      configured: true,
      lastFour: '1234',
      baseUrl: 'https://gzxsy.vip',
      apiKeyConfigured: false,
      apiKeyLastFour: null
    })),
    deleteMysteryCode: vi.fn<MacroAPI['deleteMysteryCode']>(async () => ({
      configured: false,
      lastFour: null,
      baseUrl: 'https://gzxsy.vip',
      apiKeyConfigured: false,
      apiKeyLastFour: null
    })),
    recognizeInternalSkillImage: vi.fn<MacroAPI['recognizeInternalSkillImage']>(async () => ({
      baseStats: {
        season: 0,
        strengthOrQi: 0,
        attack: 0,
        armorPenetration: 0,
        factionRestraint: 0,
        criticalHit: 0,
        maxAttack: 0,
        minAttack: 0,
        agility: 0,
        endurance: 0,
        constitution: 0
      },
      equippedSkillIds: []
    })),
    checkForUpdate: vi.fn<MacroAPI['checkForUpdate']>(async () => ({
      currentVersion: '1.7.1',
      update: null
    })),
    installUpdate: vi.fn<MacroAPI['installUpdate']>(async () => {}),
    onState: vi.fn((_callback: (nextState: MacroState) => void) => () => undefined),
    getGameRecorderState: vi.fn(async () => gameRecorderState),
    startGameRecording: vi.fn(async () => gameRecorderState),
    stopGameActivity: vi.fn(async () => gameRecorderState),
    startGamePlayback: vi.fn(async (_allowTargetMismatch?: boolean) => gameRecorderState),
    selectGameRecording: vi.fn(async (_id: string) => gameRecorderState),
    renameGameRecording: vi.fn(async (_id: string, _name: string) => gameRecorderState),
    deleteGameRecording: vi.fn(async (_id: string) => gameRecorderState),
    updateGameRecorderHotkeys: vi.fn(
      async (_hotkeys: GameRecorderState['hotkeys']) => gameRecorderState
    ),
    updateGamePlaybackSettings: vi.fn(async (_id: string, _settings) => gameRecorderState),
    onGameRecorderState: vi.fn(
      (_callback: (nextState: GameRecorderState) => void) => () => undefined
    ),
    getBuffAssistantState: vi.fn(async () => buffState),
    listBuffCaptureWindows: vi.fn(async () => []),
    listBuffSoundTemplates: vi.fn(async () => [{ id: 'template-1', name: '模板一' }]),
    captureBuffPreview: vi.fn(async () => {
      throw new Error('not configured')
    }),
    saveBuffTemplate: vi.fn(async () => buffState),
    deleteBuffTemplate: vi.fn(async () => buffState),
    updateBuffAssistantSettings: vi.fn(async () => buffState),
    startBuffMonitor: vi.fn(async () => buffState),
    stopBuffMonitor: vi.fn(async () => buffState),
    startBuffTemplateTest: vi.fn(async () => buffState),
    stopBuffTemplateTest: vi.fn(async () => buffState),
    importBuffAssistantSound: vi.fn<MacroAPI['importBuffAssistantSound']>(async () => null),
    playBuffAssistantSound: vi.fn(async () => undefined),
    openTtsOnline: vi.fn(async () => undefined),
    setBuffOverlayEditMode: vi.fn(async () => buffState),
    onBuffAssistantState: vi.fn(() => () => undefined),
    onBuffMetric: vi.fn(() => () => undefined),
    onBuffExecutionLog: vi.fn(() => () => undefined),
    onBuffOverlayState: vi.fn(() => () => undefined),
    window: {
      minimize: vi.fn(async () => undefined),
      toggleMaximize: vi.fn(async () => undefined),
      isMaximized: vi.fn(async () => false),
      close: vi.fn(async () => undefined),
      startDragging: vi.fn(async () => undefined),
      startResizeDragging: vi.fn(async () => undefined),
      onResized: vi.fn(() => () => undefined)
    }
  } satisfies MacroAPI
}

export function installMacroApi(api: MacroAPI): void {
  Object.defineProperty(window, 'api', {
    configurable: true,
    value: api,
    writable: true
  })
}

function UiProviders({ children }: { children: ReactNode }) {
  return <TooltipProvider delayDuration={0}>{children}</TooltipProvider>
}

export function renderWithUiProviders(ui: ReactElement, options?: Omit<RenderOptions, 'wrapper'>) {
  return render(ui, { wrapper: UiProviders, ...options })
}
