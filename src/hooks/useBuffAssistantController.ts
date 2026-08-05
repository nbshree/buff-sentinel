import { useCallback, useEffect, useRef, useState } from 'react'

import type {
  BuffAssistantSettings,
  BuffAssistantState,
  BuffCapturePreview,
  BuffMetric,
  CaptureWindowCandidate,
  NormalizedRect
} from '../lib/macro-api'

const defaultState: BuffAssistantState = {
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

export type BuffAssistantController = ReturnType<typeof useBuffAssistantController>

export function useBuffAssistantController() {
  const [state, setState] = useState<BuffAssistantState>(defaultState)
  const [windows, setWindows] = useState<CaptureWindowCandidate[]>([])
  const [preview, setPreview] = useState<BuffCapturePreview | null>(null)
  const [metric, setMetric] = useState<BuffMetric>({ confidence: 0, present: false })
  const [logs, setLogs] = useState<string[]>([])
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const stateRef = useRef(state)
  const lastMetricLogRef = useRef({ at: 0, present: false })

  const appendLog = useCallback((message: string) => {
    setLogs((current) => [`${formatLogTime(new Date())} ${message}`, ...current].slice(0, 150))
  }, [])

  useEffect(() => {
    let disposed = false
    void window.api
      .getBuffAssistantState()
      .then((nextState) => {
        if (!disposed) updateRuntimeState(nextState)
      })
      .catch((reason: unknown) => {
        if (!disposed) setError(toMessage(reason))
      })
    const stopState = window.api.onBuffAssistantState(updateRuntimeState)
    const stopExecutionLog = window.api.onBuffExecutionLog(appendLog)
    const stopMetric = window.api.onBuffMetric((nextMetric) => {
      setMetric(nextMetric)
      const now = Date.now()
      const statusChanged = nextMetric.present !== lastMetricLogRef.current.present
      if (statusChanged || now - lastMetricLogRef.current.at >= 1_000) {
        const runtime = stateRef.current
        const monitoring = runtime.isMonitoring && runtime.activity !== 'testing'
        const threshold = Math.round(runtime.config.settings.threshold * 100)
        const remaining = runtime.expectedAtUnixMs
          ? Math.max(0, (runtime.expectedAtUnixMs - now) / 1000)
          : null
        appendLog(
          `${monitoring ? '日常监控' : '模板测试'}：置信度 ${Math.round(
            nextMetric.confidence * 100
          )}%（阈值 ${threshold}%），${nextMetric.present ? '已确认图标' : '未确认'}${
            remaining === null ? '' : `，时间轴剩余 ${remaining.toFixed(1)} 秒`
          }`
        )
        lastMetricLogRef.current = { at: now, present: nextMetric.present }
      }
    })
    return () => {
      disposed = true
      stopState()
      stopMetric()
      stopExecutionLog()
    }

    function updateRuntimeState(nextState: BuffAssistantState): void {
      const previous = stateRef.current
      stateRef.current = nextState
      setState(nextState)
      if (nextState.activity === previous.activity) return

      if (nextState.activity === 'targetUnavailable') {
        appendLog(`游戏窗口不可用：${nextState.lastError ?? '正在等待重新连接'}`)
      } else if (nextState.activity === 'error') {
        appendLog(`识别停止：${nextState.lastError ?? '未知错误'}`)
      }
    }
  }, [appendLog])

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

  const saveTemplate = useCallback(
    async (searchRegion: NormalizedRect, crop: NormalizedRect, maskDataUrl?: string) => {
      const result = await run(() => window.api.saveBuffTemplate(searchRegion, crop, maskDataUrl))
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
    stateRef.current = result
    setState(result)
    lastMetricLogRef.current = { at: 0, present: false }
    appendLog('开始日常监控，等待脱战后下一次确认金周天')
    return result
  }, [appendLog, run])

  const stopMonitor = useCallback(async () => {
    const result = await run(() => window.api.stopBuffMonitor())
    stateRef.current = result
    setState(result)
    appendLog('停止日常监控')
    return result
  }, [appendLog, run])

  const startTest = useCallback(
    async (windowId: string) => {
      setMetric({ confidence: 0, present: false })
      lastMetricLogRef.current = { at: 0, present: false }
      const result = await run(() => window.api.startBuffTemplateTest(windowId))
      stateRef.current = result
      setState(result)
      appendLog('开始实时识别测试')
      return result
    },
    [appendLog, run]
  )

  const stopTest = useCallback(async () => {
    const result = await run(() => window.api.stopBuffTemplateTest())
    stateRef.current = result
    setState(result)
    appendLog('停止实时识别测试')
    return result
  }, [appendLog, run])

  const clearLogs = useCallback(() => setLogs([]), [])

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
    metric,
    logs,
    busy,
    error,
    setPreview,
    refreshWindows,
    capturePreview,
    saveTemplate,
    deleteTemplate,
    updateSettings,
    startMonitor,
    stopMonitor,
    startTest,
    stopTest,
    clearLogs,
    setOverlayEditing
  }
}

function formatLogTime(date: Date): string {
  return [date.getHours(), date.getMinutes(), date.getSeconds()]
    .map((part) => String(part).padStart(2, '0'))
    .join(':')
}

function toMessage(reason: unknown): string {
  if (reason instanceof Error) return reason.message
  return String(reason)
}
