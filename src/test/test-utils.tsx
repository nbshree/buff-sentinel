import { render, type RenderOptions } from '@testing-library/react'
import type { ReactElement } from 'react'
import { vi } from 'vitest'

import type { BuffAssistantState, BuffSentinelAPI } from '@/lib/buff-sentinel-api'

export function createBuffSentinelApi(buffStateOverride?: BuffAssistantState) {
  const buffState: BuffAssistantState = buffStateOverride ?? {
    config: {
      schemaVersion: 10,
      target: null,
      searchRegion: null,
      listeners: [],
      settings: {
        overlay: {
          x: 40,
          y: 100,
          showWaitingDot: false,
          excludeFromCapture: false,
          width: 330,
          height: 92,
          colorScheme: 'gold'
        },
        capture: {
          showSystemBorder: true
        }
      }
    },
    activity: 'stopped',
    isMonitoring: false,
    listeners: [],
    lastError: null,
    captureBorderSupported: true,
    captureBorderNotice: null
  }

  return {
    getAppVersion: vi.fn(async () => '0.1.0'),
    checkForUpdate: vi.fn(async () => ({ currentVersion: '0.1.0', update: null })),
    installUpdate: vi.fn(async () => undefined),
    getBuffAssistantState: vi.fn(async () => buffState),
    listBuffCaptureWindows: vi.fn(async () => []),
    listBuffSoundTemplates: vi.fn(async () => [{ id: 'template-1', name: '模板一' }]),
    captureBuffPreview: vi.fn(async () => {
      throw new Error('not configured')
    }),
    updateBuffSearchRegion: vi.fn(async () => buffState),
    getBuffListenerTemplate: vi.fn(async () => ({
      imageDataUrl: 'data:image/png;base64,dGVtcGxhdGU=',
      maskDataUrl: 'data:image/png;base64,bWFzaw==',
      crop: { x: 0.2, y: 0.2, width: 0.3, height: 0.3 }
    })),
    saveBuffListener: vi.fn(async () => buffState),
    updateBuffListener: vi.fn(async () => buffState),
    deleteBuffListener: vi.fn(async () => buffState),
    updateBuffAssistantSettings: vi.fn(async () => buffState),
    requestBuffBorderlessCaptureAccess: vi.fn<
      BuffSentinelAPI['requestBuffBorderlessCaptureAccess']
    >(async () => 'allowed'),
    startBuffMonitor: vi.fn(async () => buffState),
    stopBuffMonitor: vi.fn(async () => buffState),
    startBuffTemplateTest: vi.fn(async () => buffState),
    stopBuffTemplateTest: vi.fn(async () => buffState),
    importBuffAssistantSound: vi.fn<BuffSentinelAPI['importBuffAssistantSound']>(async () => null),
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
  } satisfies BuffSentinelAPI
}

export function installBuffSentinelApi(api: BuffSentinelAPI): void {
  Object.defineProperty(window, 'api', {
    configurable: true,
    value: api,
    writable: true
  })
}

export function renderWithUiProviders(ui: ReactElement, options?: RenderOptions) {
  return render(ui, options)
}
