import { useCallback, useRef, useState } from 'react'

import type {
  AppUpdateCheckResult,
  AppUpdateDownloadEvent,
  AppUpdateInfo
} from '../lib/buff-sentinel-api'

export type AppUpdaterStatus =
  'idle' | 'checking' | 'upToDate' | 'available' | 'downloading' | 'installing' | 'error'

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
  isBusy: boolean
  checkOnStartup: () => Promise<void>
  checkForUpdate: () => Promise<void>
  installUpdate: () => Promise<void>
  retry: () => Promise<void>
  setOpen: (open: boolean) => void
}

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message
  if (typeof error === 'string' && error.trim()) return error
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = error.message
    if (typeof message === 'string' && message.trim()) return message
  }
  return '请检查网络连接后重试。'
}

export function useAppUpdater(installBlockedReason: string | null): AppUpdaterController {
  const [open, setOpenState] = useState(false)
  const [status, setStatus] = useState<AppUpdaterStatus>('idle')
  const [result, setResult] = useState<AppUpdateCheckResult | null>(null)
  const [downloaded, setDownloaded] = useState(0)
  const [total, setTotal] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)
  const failedOperation = useRef<'check' | 'install'>('check')
  const operationInProgress = useRef(false)

  const setOpen = useCallback((nextOpen: boolean) => {
    if (!nextOpen && operationInProgress.current) return
    setOpenState(nextOpen)
  }, [])

  const performCheck = useCallback(async (showWhileChecking: boolean) => {
    if (operationInProgress.current) return
    operationInProgress.current = true
    failedOperation.current = 'check'
    if (showWhileChecking) setOpenState(true)
    setStatus('checking')
    setError(null)
    try {
      const nextResult = await window.api.checkForUpdate()
      setResult(nextResult)
      setStatus(nextResult.update ? 'available' : 'upToDate')
      if (nextResult.update) setOpenState(true)
    } catch (reason) {
      setStatus('error')
      setError(`检查更新失败：${getErrorMessage(reason)}`)
      if (showWhileChecking) setOpenState(true)
    } finally {
      operationInProgress.current = false
    }
  }, [])

  const installUpdate = useCallback(async () => {
    if (operationInProgress.current || !result?.update || installBlockedReason) return
    operationInProgress.current = true
    failedOperation.current = 'install'
    setStatus('downloading')
    setDownloaded(0)
    setTotal(null)
    setError(null)
    try {
      await window.api.installUpdate((event: AppUpdateDownloadEvent) => {
        setDownloaded(event.downloaded)
        setTotal(event.total)
        setStatus(event.event === 'finished' ? 'installing' : 'downloading')
      })
      setStatus('installing')
    } catch (reason) {
      setStatus('error')
      setError(`安装更新失败：${getErrorMessage(reason)}`)
    } finally {
      operationInProgress.current = false
    }
  }, [installBlockedReason, result?.update])

  const checkOnStartup = useCallback(() => performCheck(false), [performCheck])
  const checkForUpdate = useCallback(() => performCheck(true), [performCheck])
  const retry = useCallback(
    () => (failedOperation.current === 'install' ? installUpdate() : performCheck(true)),
    [installUpdate, performCheck]
  )

  return {
    open,
    status,
    currentVersion: result?.currentVersion ?? null,
    update: result?.update ?? null,
    downloaded,
    total,
    progressPercent:
      total && total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : null,
    error,
    installBlockedReason,
    isBusy: status === 'checking' || status === 'downloading' || status === 'installing',
    checkOnStartup,
    checkForUpdate,
    installUpdate,
    retry,
    setOpen
  }
}
