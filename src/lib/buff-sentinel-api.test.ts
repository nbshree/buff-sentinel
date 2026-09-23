import { afterEach, describe, expect, it, vi } from 'vitest'

import type { BuffOverlayState } from './buff-sentinel-api'

const { invoke, listen, unlisten, currentWindow } = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  unlisten: vi.fn(),
  currentWindow: { label: 'buff-overlay' }
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke, Channel: class {} }))
vi.mock('@tauri-apps/api/event', () => ({ listen }))
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => currentWindow }))

import { buffSentinelApi } from './buff-sentinel-api'

const waiting: BuffOverlayState = {
  mode: 'waiting',
  message: '',
  items: [],
  emittedAtUnixMs: 1,
  editable: false,
  colorScheme: 'blackWhite'
}

afterEach(() => {
  invoke.mockReset()
  listen.mockReset()
  unlisten.mockReset()
  currentWindow.label = 'buff-overlay'
})

describe('overlay state subscription', () => {
  it('recovers the state emitted before the overlay subscribed', async () => {
    listen.mockResolvedValue(unlisten)
    invoke.mockResolvedValue(waiting)
    const callback = vi.fn()

    const stop = buffSentinelApi.onBuffOverlayState(callback)

    await vi.waitFor(() => expect(callback).toHaveBeenCalledWith(waiting))
    expect(listen).toHaveBeenCalledWith('buff-overlay-state', expect.any(Function), {
      target: 'buff-overlay'
    })
    expect(invoke).toHaveBeenCalledWith('get_buff_overlay_state')
    stop()
    expect(unlisten).toHaveBeenCalledOnce()
  })

  it('does not replace a newer event with an older snapshot', async () => {
    let resolveSnapshot!: (state: BuffOverlayState) => void
    invoke.mockReturnValue(
      new Promise((resolve) => {
        resolveSnapshot = resolve
      })
    )
    listen.mockResolvedValue(unlisten)
    const callback = vi.fn()
    const stop = buffSentinelApi.onBuffOverlayState(callback)

    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('get_buff_overlay_state'))
    const newer = { ...waiting, mode: 'countdown', emittedAtUnixMs: 2 } as BuffOverlayState
    listen.mock.calls[0][1]({ payload: newer })
    resolveSnapshot(waiting)
    await Promise.resolve()
    await Promise.resolve()
    expect(callback).toHaveBeenCalledWith(newer)
    expect(callback).toHaveBeenCalledTimes(1)
    stop()
  })

  it('subscribes the skill overlay only to its own window target', async () => {
    currentWindow.label = 'buff-overlay-skill'
    listen.mockResolvedValue(unlisten)
    invoke.mockResolvedValue(waiting)

    const stop = buffSentinelApi.onBuffOverlayState(vi.fn())

    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('get_buff_overlay_state'))
    expect(listen).toHaveBeenCalledWith('buff-overlay-state', expect.any(Function), {
      target: 'buff-overlay-skill'
    })
    stop()
  })
})
