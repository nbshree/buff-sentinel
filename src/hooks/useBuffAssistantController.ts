import { useCallback, useEffect, useState } from 'react'

import type {
  BuffAssistantSettings,
  BuffAssistantState,
  BuffCapturePreview,
  BuffMetric,
  BuffSampleFrameSummary,
  CaptureWindowCandidate,
  NormalizedRect
} from '../lib/macro-api'

const defaultState: BuffAssistantState = {
  config: {
    schemaVersion: 1,
    target: null,
    searchRegion: null,
    template: null,
    settings: {
      cycleMs: 20_000,
      threshold: 0.86,
      confirmFrames: 3,
      missingFrames: 5,
      sound: {
        triggerEnabled: true,
        prewarnThreeEnabled: true,
        prewarnOneEnabled: true,
        volume: 0.45
      },
      overlay: { x: 40, y: 100, showWaitingDot: false }
    }
  },
  activity: 'stopped',
  isMonitoring: false,
  expectedAtUnixMs: null,
  lastConfidence: 0,
  sampleCount: 0,
  lastError: null
}

export type BuffAssistantController = ReturnType<typeof useBuffAssistantController>

export function useBuffAssistantController() {
  const [state, setState] = useState<BuffAssistantState>(defaultState)
  const [windows, setWindows] = useState<CaptureWindowCandidate[]>([])
  const [preview, setPreview] = useState<BuffCapturePreview | null>(null)
  const [samples, setSamples] = useState<BuffSampleFrameSummary[]>([])
  const [selectedFrame, setSelectedFrame] = useState<string | null>(null)
  const [metric, setMetric] = useState<BuffMetric>({ confidence: 0, present: false })
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let disposed = false
    void window.api
      .getBuffAssistantState()
      .then((nextState) => {
        if (!disposed) setState(nextState)
      })
      .catch((reason: unknown) => {
        if (!disposed) setError(toMessage(reason))
      })
    const stopState = window.api.onBuffAssistantState(setState)
    const stopMetric = window.api.onBuffMetric(setMetric)
    return () => {
      disposed = true
      stopState()
      stopMetric()
    }
  }, [])

  const run = useCallback(async <T>(operation: () => Promise<T>): Promise<T> => {
    setBusy(true)
    setError(null)
    try {
      return await operation()
    } catch (reason) {
      const message = toMessage(reason)
      setError(message)
      throw reason
    } finally {
      setBusy(false)
    }
  }, [])

  const refreshWindows = useCallback(async () => {
    const items = await run(() => window.api.listBuffCaptureWindows())
    setWindows(items)
    return items
  }, [run])

  const capturePreview = useCallback(
    async (windowId: string) => {
      const result = await run(() => window.api.captureBuffPreview(windowId))
      setPreview(result)
      return result
    },
    [run]
  )

  const startSampleCapture = useCallback(
    async (windowId: string, region: NormalizedRect) => {
      const result = await run(() => window.api.startBuffSampleCapture(windowId, region))
      setState(result)
      setSamples([])
      setSelectedFrame(null)
      return result
    },
    [run]
  )

  const pauseSampleCapture = useCallback(async () => {
    const result = await run(() => window.api.pauseBuffSampleCapture())
    setState(result)
    return result
  }, [run])

  const loadSamples = useCallback(async () => {
    const frames = await run(() => window.api.listBuffSampleFrames())
    setSamples(frames)
    return frames
  }, [run])

  const loadSampleFrame = useCallback(
    async (id: number) => {
      const frame = await run(() => window.api.getBuffSampleFrame(id))
      setSelectedFrame(frame)
      return frame
    },
    [run]
  )

  const saveTemplate = useCallback(
    async (sampleId: number, crop: NormalizedRect, maskDataUrl?: string) => {
      const result = await run(() => window.api.saveBuffTemplate(sampleId, crop, maskDataUrl))
      setState(result)
      return result
    },
    [run]
  )

  const deleteTemplate = useCallback(async () => {
    const result = await run(() => window.api.deleteBuffTemplate())
    setState(result)
    return result
  }, [run])

  const updateSettings = useCallback(
    async (settings: BuffAssistantSettings) => {
      const result = await run(() => window.api.updateBuffAssistantSettings(settings))
      setState(result)
      return result
    },
    [run]
  )

  const startMonitor = useCallback(async () => {
    const result = await run(() => window.api.startBuffMonitor())
    setState(result)
    return result
  }, [run])

  const stopMonitor = useCallback(async () => {
    const result = await run(() => window.api.stopBuffMonitor())
    setState(result)
    return result
  }, [run])

  const startTest = useCallback(
    async (windowId: string) => {
      setMetric({ confidence: 0, present: false })
      const result = await run(() => window.api.startBuffTemplateTest(windowId))
      setState(result)
      return result
    },
    [run]
  )

  const stopTest = useCallback(async () => {
    const result = await run(() => window.api.stopBuffTemplateTest())
    setState(result)
    return result
  }, [run])

  const setOverlayEditing = useCallback(
    async (enabled: boolean) => {
      const result = await run(() => window.api.setBuffOverlayEditMode(enabled))
      setState(result)
      return result
    },
    [run]
  )

  return {
    state,
    windows,
    preview,
    samples,
    selectedFrame,
    metric,
    busy,
    error,
    setPreview,
    setSelectedFrame,
    refreshWindows,
    capturePreview,
    startSampleCapture,
    pauseSampleCapture,
    loadSamples,
    loadSampleFrame,
    saveTemplate,
    deleteTemplate,
    updateSettings,
    startMonitor,
    stopMonitor,
    startTest,
    stopTest,
    setOverlayEditing
  }
}

function toMessage(reason: unknown): string {
  if (reason instanceof Error) return reason.message
  return String(reason)
}
