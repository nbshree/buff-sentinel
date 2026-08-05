import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'

import { createMacroApi, installMacroApi } from '../test/test-utils'
import { useAppUpdater } from './useAppUpdater'

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve
    reject = nextReject
  })
  return { promise, reject, resolve }
}

describe('useAppUpdater', () => {
  beforeEach(() => {
    installMacroApi(createMacroApi())
  })

  it('checks on startup without opening when the app is up-to-date', async () => {
    const api = createMacroApi()
    installMacroApi(api)
    const { result } = renderHook(() => useAppUpdater({ installBlockedReason: null }))

    await act(async () => {
      await result.current.checkOnStartup()
    })

    expect(api.checkForUpdate).toHaveBeenCalledTimes(1)
    expect(result.current.open).toBe(false)
    expect(result.current.status).toBe('upToDate')
    expect(result.current.currentVersion).toBe('1.7.1')
  })

  it('opens the dialog when the startup check finds an update', async () => {
    const api = createMacroApi()
    api.checkForUpdate.mockResolvedValue({
      currentVersion: '1.7.1',
      update: { version: '1.8.0', notes: '新增应用内更新', publishedAt: null }
    })
    installMacroApi(api)
    const { result } = renderHook(() => useAppUpdater({ installBlockedReason: null }))

    await act(async () => {
      await result.current.checkOnStartup()
    })

    expect(result.current.open).toBe(true)
    expect(result.current.status).toBe('available')
  })

  it('keeps startup check failures in the background', async () => {
    const api = createMacroApi()
    api.checkForUpdate.mockRejectedValue({ code: 'checkFailed', message: '网络不可用' })
    installMacroApi(api)
    const { result } = renderHook(() => useAppUpdater({ installBlockedReason: null }))

    await act(async () => {
      await result.current.checkOnStartup()
    })

    expect(result.current.open).toBe(false)
    expect(result.current.status).toBe('error')
  })

  it('keeps the new version metadata returned by the backend', async () => {
    const api = createMacroApi()
    api.checkForUpdate.mockResolvedValue({
      currentVersion: '1.7.1',
      update: {
        version: '1.8.0',
        notes: '新增应用内更新',
        publishedAt: '2026-07-21T12:00:00Z'
      }
    })
    installMacroApi(api)
    const { result } = renderHook(() => useAppUpdater({ installBlockedReason: null }))

    await act(async () => {
      await result.current.checkForUpdate()
    })

    expect(result.current.status).toBe('available')
    expect(result.current.update).toEqual({
      version: '1.8.0',
      notes: '新增应用内更新',
      publishedAt: '2026-07-21T12:00:00Z'
    })
  })

  it('protects against repeated checks while a request is pending', async () => {
    const pending = deferred<Awaited<ReturnType<typeof window.api.checkForUpdate>>>()
    const api = createMacroApi()
    api.checkForUpdate.mockImplementation(() => pending.promise)
    installMacroApi(api)
    const { result } = renderHook(() => useAppUpdater({ installBlockedReason: null }))

    act(() => {
      void result.current.checkForUpdate()
      void result.current.checkForUpdate()
    })

    expect(api.checkForUpdate).toHaveBeenCalledTimes(1)

    await act(async () => {
      pending.resolve({ currentVersion: '1.7.1', update: null })
      await pending.promise
    })

    expect(result.current.status).toBe('upToDate')
  })

  it('retries a failed check', async () => {
    const api = createMacroApi()
    api.checkForUpdate
      .mockRejectedValueOnce({ code: 'checkFailed', message: '网络不可用' })
      .mockResolvedValueOnce({ currentVersion: '1.7.1', update: null })
    installMacroApi(api)
    const { result } = renderHook(() => useAppUpdater({ installBlockedReason: null }))

    await act(async () => {
      await result.current.checkForUpdate()
    })
    expect(result.current.status).toBe('error')
    expect(result.current.error).toContain('网络不可用')

    await act(async () => {
      await result.current.retry()
    })

    expect(api.checkForUpdate).toHaveBeenCalledTimes(2)
    expect(result.current.status).toBe('upToDate')
  })

  it('reports cumulative download progress and switches to installing', async () => {
    const installFinished = deferred<void>()
    const api = createMacroApi()
    api.checkForUpdate.mockResolvedValue({
      currentVersion: '1.7.1',
      update: { version: '1.8.0', notes: '', publishedAt: null }
    })
    api.installUpdate.mockImplementation(async (onEvent) => {
      onEvent({ event: 'started', downloaded: 0, total: 200 })
      onEvent({ event: 'progress', downloaded: 50, total: 200 })
      return installFinished.promise
    })
    installMacroApi(api)
    const { result } = renderHook(() => useAppUpdater({ installBlockedReason: null }))

    await act(async () => {
      await result.current.checkForUpdate()
    })
    act(() => {
      void result.current.installUpdate()
      void result.current.installUpdate()
    })

    await waitFor(() => expect(result.current.progressPercent).toBe(25))
    expect(api.installUpdate).toHaveBeenCalledTimes(1)
    expect(result.current.downloaded).toBe(50)
    expect(result.current.total).toBe(200)
    expect(result.current.status).toBe('downloading')

    await act(async () => {
      installFinished.resolve()
      await installFinished.promise
    })
    expect(result.current.status).toBe('installing')
  })

  it('retries an installation after a download failure', async () => {
    const api = createMacroApi()
    api.checkForUpdate.mockResolvedValue({
      currentVersion: '1.7.1',
      update: { version: '1.8.0', notes: '', publishedAt: null }
    })
    api.installUpdate
      .mockRejectedValueOnce({ code: 'downloadFailed', message: '下载连接已中断' })
      .mockResolvedValueOnce(undefined)
    installMacroApi(api)
    const { result, rerender } = renderHook(
      ({ installBlockedReason }) => useAppUpdater({ installBlockedReason }),
      { initialProps: { installBlockedReason: null as string | null } }
    )

    await act(async () => {
      await result.current.checkForUpdate()
    })
    await act(async () => {
      await result.current.installUpdate()
    })

    expect(result.current.status).toBe('error')
    expect(result.current.error).toContain('下载连接已中断')

    rerender({ installBlockedReason: '宏正在执行，请先停止执行再安装更新。' })
    expect(result.current.retryBlockedReason).toBe('宏正在执行，请先停止执行再安装更新。')

    await act(async () => {
      await result.current.retry()
    })
    expect(api.installUpdate).toHaveBeenCalledTimes(1)

    rerender({ installBlockedReason: null })
    expect(result.current.retryBlockedReason).toBeNull()

    await act(async () => {
      await result.current.retry()
    })

    expect(api.installUpdate).toHaveBeenCalledTimes(2)
    expect(result.current.status).toBe('installing')
  })

  it.each([
    '宏正在执行，请先停止执行再安装更新。',
    '当前有未保存的编辑，请先保存或撤销后再安装更新。'
  ])('does not install while blocked: %s', async (installBlockedReason) => {
    const api = createMacroApi()
    api.checkForUpdate.mockResolvedValue({
      currentVersion: '1.7.1',
      update: { version: '1.8.0', notes: '', publishedAt: null }
    })
    installMacroApi(api)
    const { result } = renderHook(() => useAppUpdater({ installBlockedReason }))

    await act(async () => {
      await result.current.checkForUpdate()
    })
    expect(result.current.status).toBe('available')

    await act(async () => {
      await result.current.installUpdate()
    })

    expect(api.installUpdate).not.toHaveBeenCalled()
    expect(result.current.status).toBe('available')
  })
})
