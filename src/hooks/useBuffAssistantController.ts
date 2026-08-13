import { useCallback, useEffect, useRef, useState } from 'react'

import type {
  BorderlessCaptureAccessResult,
  BuffAssistantState,
  BuffCapturePreview,
  BuffGlobalSettings,
  BuffListenerSettings,
  BuffMetric,
  CaptureWindowCandidate,
  NormalizedRect
} from '../lib/buff-sentinel-api'

const defaultState: BuffAssistantState = {
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
  captureBorderSupported: false,
  captureBorderNotice: null
}

export type BuffAssistantController = ReturnType<typeof useBuffAssistantController>

export function useBuffAssistantController() {
  const [state, setState] = useState<BuffAssistantState>(defaultState)
  const [windows, setWindows] = useState<CaptureWindowCandidate[]>([])
  const [preview, setPreview] = useState<BuffCapturePreview | null>(null)
  const [metrics, setMetrics] = useState<Record<string, BuffMetric>>({})
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
    const stopMetric = window.api.onBuffMetric(({ metrics: nextMetrics }) => {
      setMetrics((current) => {
        const next = { ...current }
        for (const metric of nextMetrics) next[metric.listenerId] = metric
        return next
      })
      const now = Date.now()
      const primaryMetric = nextMetrics[0]
      if (!primaryMetric) return
      const statusChanged = primaryMetric.present !== lastMetricLogRef.current.present
      if (statusChanged || now - lastMetricLogRef.current.at >= 1_000) {
        const runtime = stateRef.current
        const monitoring = runtime.isMonitoring && runtime.activity !== 'testing'
        const listener = runtime.config.listeners.find((item) => item.id === primaryMetric.listenerId)
        const listenerRuntime = runtime.listeners.find((item) => item.id === primaryMetric.listenerId)
        const threshold = Math.round((listener?.settings.threshold ?? 0) * 100)
        const remaining = listenerRuntime?.expectedAtUnixMs
          ? Math.max(0, (listenerRuntime.expectedAtUnixMs - now) / 1000)
          : null
        appendLog(
          `${listener?.name ?? '监听项'} · ${monitoring ? '监控' : '模板测试'}：置信度 ${Math.round(
            primaryMetric.confidence * 100
          )}%（阈值 ${threshold}%），${primaryMetric.present ? '已确认图标' : '未确认'}${
            remaining === null ? '' : `，时间轴剩余 ${remaining.toFixed(1)} 秒`
          }`
        )
        lastMetricLogRef.current = { at: now, present: primaryMetric.present }
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

  const updateBuffSearchRegion = useCallback(
    async (searchRegion: NormalizedRect) => {
      const result = await run(() => window.api.updateBuffSearchRegion(searchRegion))
      stateRef.current = result
      setState(result)
      return result
    },
    [run]
  )

  const getListenerTemplate = useCallback(
    (listenerId: string) => run(() => window.api.getBuffListenerTemplate(listenerId)),
    [run]
  )

  const saveListener = useCallback(
    async (
      listenerId: string | null,
      name: string,
      enabled: boolean,
      settings: BuffListenerSettings,
      searchRegion: NormalizedRect,
      crop: NormalizedRect,
      maskDataUrl?: string
    ) => {
      const result = await run(() =>
        window.api.saveBuffListener(
          listenerId,
          name,
          enabled,
          settings,
          searchRegion,
          crop,
          maskDataUrl
        )
      )
      setState(result)
      return result
    },
    [run]
  )

  const updateListener = useCallback(async (
    listenerId: string,
    name: string,
    enabled: boolean,
    settings: BuffListenerSettings,
    maskDataUrl?: string,
    crop?: NormalizedRect
  ) => {
    const result = await run(() =>
      maskDataUrl === undefined && crop === undefined
        ? window.api.updateBuffListener(listenerId, name, enabled, settings)
        : window.api.updateBuffListener(listenerId, name, enabled, settings, maskDataUrl, crop)
    )
    setState(result)
    return result
  }, [run])

  const deleteListener = useCallback(async (listenerId: string) => {
    const result = await run(() => window.api.deleteBuffListener(listenerId))
    setState(result)
    return result
  }, [run])

  const updateSettings = useCallback(
    async (settings: BuffGlobalSettings) => {
      const result = await run(() => window.api.updateBuffAssistantSettings(settings))
      setState(result)
      return result
    },
    [run]
  )

  const requestBorderlessCaptureAccess = useCallback(async () => {
    const result = await run(() => window.api.requestBuffBorderlessCaptureAccess())
    setState((current) => ({
      ...current,
      captureBorderNotice: borderlessAccessNotice(result)
    }))
    return result
  }, [run])

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
    async (windowId: string, listenerId: string) => {
      setMetrics((current) => ({
        ...current,
        [listenerId]: { listenerId, confidence: 0, present: false }
      }))
      lastMetricLogRef.current = { at: 0, present: false }
      const result = await run(() => window.api.startBuffTemplateTest(windowId, listenerId))
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
    metrics,
    logs,
    busy,
    error,
    setPreview,
    refreshWindows,
    capturePreview,
    updateBuffSearchRegion,
    getListenerTemplate,
    saveListener,
    updateListener,
    deleteListener,
    updateSettings,
    requestBorderlessCaptureAccess,
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

function borderlessAccessNotice(result: BorderlessCaptureAccessResult): string | null {
  const notices = {
    allowed: null,
    unsupported: '当前 Windows 版本不支持隐藏系统捕获黄色边框',
    deniedByUser: '未获得隐藏系统捕获边框的用户授权，已继续显示黄色边框',
    deniedBySystem: 'Windows 未允许隐藏系统捕获边框，已继续显示黄色边框',
    notDeclared: '当前应用安装方式不允许隐藏系统捕获边框'
  } satisfies Record<typeof result, string | null>
  return notices[result]
}
