import { useCallback, useRef, useState } from 'react'

import type { AppUpdateCheckResult, AppUpdateDownloadEvent, AppUpdateInfo } from '../lib/macro-api'

export type AppUpdaterStatus =
  'idle' | 'checking' | 'upToDate' | 'available' | 'downloading' | 'installing' | 'error'

type FailedOperation = 'check' | 'install'

export type AppUpdaterController = {
  open: boolean
  status: AppUpdaterStatus
  currentVersion: string | null
  update: AppUpdateInfo | null
  downloaded: number
  total: number | null
  progressPercent: number | null
  error: string | null
  installBlockedReason: string | null
  retryBlockedReason: string | null
  isBusy: boolean
  checkOnStartup: () => Promise<void>
  checkForUpdate: () => Promise<void>
  installUpdate: () => Promise<void>
  retry: () => Promise<void>
  setOpen: (open: boolean) => void
}

type UseAppUpdaterOptions = {
  installBlockedReason: string | null
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message
  if (typeof error === 'string' && error.trim()) return error
  if (
    typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof error.message === 'string' &&
    error.message.trim()
  ) {
    return error.message
  }
  return '请检查网络连接后重试。'
}

function getProgressPercent(downloaded: number, total: number | null): number | null {
  if (total === null || total <= 0) return null
  return Math.min(100, Math.max(0, Math.round((downloaded / total) * 100)))
}

function describeFailure(prefix: string, error: unknown): string {
  const message = getErrorMessage(error)
  return message.startsWith(prefix) ? message : `${prefix}：${message}`
}

export function useAppUpdater({
  installBlockedReason
}: UseAppUpdaterOptions): AppUpdaterController {
  const [open, setOpenState] = useState(false)
  const [status, setStatus] = useState<AppUpdaterStatus>('idle')
  const [result, setResult] = useState<AppUpdateCheckResult | null>(null)
  const [downloaded, setDownloaded] = useState(0)
  const [total, setTotal] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)
  const failedOperationRef = useRef<FailedOperation>('check')
  const operationInProgressRef = useRef(false)

  const setOpen = useCallback(
    (nextOpen: boolean) => {
      if (!nextOpen && (status === 'downloading' || status === 'installing')) return
      setOpenState(nextOpen)
    },
    [status]
  )

  const performCheck = useCallback(async (openBeforeCheck: boolean) => {
    if (operationInProgressRef.current) return

    operationInProgressRef.current = true
    failedOperationRef.current = 'check'
    if (openBeforeCheck) setOpenState(true)
    setStatus('checking')
    setResult(null)
    setDownloaded(0)
    setTotal(null)
    setError(null)

    try {
      const nextResult = await window.api.checkForUpdate()
      setResult(nextResult)
      setStatus(nextResult.update ? 'available' : 'upToDate')
      if (nextResult.update) setOpenState(true)
    } catch (nextError) {
      setStatus('error')
      setError(describeFailure('检查更新失败', nextError))
      if (openBeforeCheck) setOpenState(true)
    } finally {
      operationInProgressRef.current = false
    }
  }, [])

  const checkOnStartup = useCallback(async () => {
    await performCheck(false)
  }, [performCheck])

  const checkForUpdate = useCallback(async () => {
    await performCheck(true)
  }, [performCheck])

  const installUpdate = useCallback(async () => {
    if (operationInProgressRef.current || !result?.update || installBlockedReason) return

    operationInProgressRef.current = true
    failedOperationRef.current = 'install'
    setStatus('downloading')
    setDownloaded(0)
    setTotal(null)
    setError(null)

    function handleEvent(event: AppUpdateDownloadEvent): void {
      setDownloaded(Math.max(0, event.downloaded))
      setTotal(event.total === null ? null : Math.max(0, event.total))
      setStatus(event.event === 'finished' ? 'installing' : 'downloading')
    }

    try {
      await window.api.installUpdate(handleEvent)
      setStatus('installing')
    } catch (nextError) {
      setStatus('error')
      setError(describeFailure('安装更新失败', nextError))
    } finally {
      operationInProgressRef.current = false
    }
  }, [installBlockedReason, result?.update])

  const retry = useCallback(async () => {
    if (failedOperationRef.current === 'install') await installUpdate()
    else await checkForUpdate()
  }, [checkForUpdate, installUpdate])

  return {
    open,
    status,
    currentVersion: result?.currentVersion ?? null,
    update: result?.update ?? null,
    downloaded,
    total,
    progressPercent: getProgressPercent(downloaded, total),
    error,
    installBlockedReason,
    retryBlockedReason:
      status === 'error' && failedOperationRef.current === 'install' ? installBlockedReason : null,
    isBusy: status === 'checking' || status === 'downloading' || status === 'installing',
    checkOnStartup,
    checkForUpdate,
    installUpdate,
    retry,
    setOpen
  }
}
