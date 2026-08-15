import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { BuffOverlayState, BuffSentinelAPI } from '@/lib/buff-sentinel-api'
import { createBuffSentinelApi, installBuffSentinelApi } from '@/test/test-utils'

import { BuffOverlayApp, calculateOverlayScale } from './BuffOverlayApp'

let emitOverlayState: (state: BuffOverlayState) => void

function renderOverlay(): BuffSentinelAPI {
  const api: BuffSentinelAPI = createBuffSentinelApi()
  api.onBuffOverlayState = vi.fn((callback) => {
    emitOverlayState = callback
    return () => undefined
  })
  installBuffSentinelApi(api)
  render(<BuffOverlayApp />)
  return api
}

function emit(state: BuffOverlayState): void {
  act(() => emitOverlayState(state))
}

describe('BuffOverlayApp', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-08-03T12:00:00.000Z'))
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('shows the confirmation message without a countdown', () => {
    renderOverlay()

    emit({
      mode: 'confirming',
      message: '等待金周天确认',
      items: [],
      emittedAtUnixMs: Date.now(),
      editable: false,
      colorScheme: 'gold'
    })

    expect(screen.getByText('等待金周天确认')).toBeInTheDocument()
    expect(screen.queryByText('秒')).not.toBeInTheDocument()
    expect(document.querySelector('.buff-overlay__countdown')).not.toBeInTheDocument()
  })

  it('starts a fresh countdown from the expected time in the next event', () => {
    renderOverlay()

    emit({
      mode: 'confirming',
      message: '等待金周天确认',
      items: [],
      emittedAtUnixMs: Date.now(),
      editable: false,
      colorScheme: 'gold'
    })
    emit({
      mode: 'countdown',
      message: '',
      items: [
        {
          listenerId: 'jinzhoutian',
          name: '金周天',
          mode: 'countdown',
          expectedAtUnixMs: Date.now() + 20_000
        }
      ],
      emittedAtUnixMs: Date.now(),
      editable: false,
      colorScheme: 'gold'
    })

    expect(screen.getByText('金周天')).toBeInTheDocument()
    expect(screen.getByText('20.0')).toBeInTheDocument()

    act(() => vi.advanceTimersByTime(500))

    expect(screen.getByText('19.5')).toBeInTheDocument()
  })

  it('updates the countdown every 100 milliseconds and clears the timer on unmount', () => {
    const api: BuffSentinelAPI = createBuffSentinelApi()
    api.onBuffOverlayState = vi.fn((callback) => {
      emitOverlayState = callback
      return () => undefined
    })
    installBuffSentinelApi(api)
    const view = render(<BuffOverlayApp />)

    emit({
      mode: 'countdown',
      message: '',
      items: [
        {
          listenerId: 'jinzhoutian',
          name: '金周天',
          mode: 'countdown',
          expectedAtUnixMs: Date.now() + 20_000
        }
      ],
      emittedAtUnixMs: Date.now(),
      editable: false,
      colorScheme: 'gold'
    })

    expect(screen.getByText('20.0')).toBeInTheDocument()
    act(() => vi.advanceTimersByTime(99))
    expect(screen.getByText('20.0')).toBeInTheDocument()
    act(() => vi.advanceTimersByTime(1))
    expect(screen.getByText('19.9')).toBeInTheDocument()
    expect(vi.getTimerCount()).toBe(1)

    view.unmount()
    expect(vi.getTimerCount()).toBe(0)
  })

  it('keeps the countdown text stable while previewing the editable overlay', () => {
    renderOverlay()

    emit({
      mode: 'countdown',
      message: '',
      items: [
        {
          listenerId: 'jinzhoutian',
          name: '金周天',
          mode: 'countdown',
          expectedAtUnixMs: Date.now() + 20_000
        }
      ],
      emittedAtUnixMs: Date.now(),
      editable: true,
      colorScheme: 'gold'
    })

    expect(screen.getByText('20.0')).toBeInTheDocument()
    act(() => vi.advanceTimersByTime(500))
    expect(screen.getByText('20.0')).toBeInTheDocument()
  })

  it('shows the simplified waiting message', () => {
    renderOverlay()

    emit({
      mode: 'waiting',
      message: '等待金周天',
      items: [],
      emittedAtUnixMs: Date.now(),
      editable: false,
      colorScheme: 'gold'
    })

    expect(screen.getByText('等待金周天')).toBeInTheDocument()
    expect(screen.queryByText(/脱战/)).not.toBeInTheDocument()
  })

  it('shows an independent waiting row for every listener', () => {
    renderOverlay()

    emit({
      mode: 'waiting',
      message: '',
      items: [
        {
          listenerId: 'jinzhoutian',
          name: '金周天',
          mode: 'waiting',
          expectedAtUnixMs: null
        },
        {
          listenerId: 'listener-2',
          name: '监听图标 2',
          mode: 'waiting',
          expectedAtUnixMs: null
        }
      ],
      emittedAtUnixMs: Date.now(),
      editable: true,
      colorScheme: 'gold'
    })

    expect(screen.getByText('金周天')).toBeInTheDocument()
    expect(screen.getByText('监听图标 2')).toBeInTheDocument()
    expect(screen.getAllByText('等待监听')).toHaveLength(2)
  })

  it('only exposes resize handles while editing and keeps resize separate from dragging', () => {
    const api = renderOverlay()

    emit({
      mode: 'editing',
      message: '拖动调整位置，右侧调整宽度',
      items: [],
      emittedAtUnixMs: Date.now(),
      editable: true,
      colorScheme: 'blackWhite'
    })

    fireEvent.pointerDown(screen.getByRole('button', { name: '调整浮窗宽度' }), { button: 0 })
    expect(api.window.startResizeDragging).toHaveBeenNthCalledWith(1, 'East')
    fireEvent.pointerDown(
      screen.getByRole('button', { name: '从左上角调整浮窗大小' }),
      { button: 0 }
    )
    fireEvent.pointerDown(
      screen.getByRole('button', { name: '从右上角调整浮窗大小' }),
      { button: 0 }
    )
    fireEvent.pointerDown(
      screen.getByRole('button', { name: '从左下角调整浮窗大小' }),
      { button: 0 }
    )
    fireEvent.pointerDown(
      screen.getByRole('button', { name: '从右下角调整浮窗大小' }),
      { button: 0 }
    )
    expect(api.window.startResizeDragging).toHaveBeenNthCalledWith(2, 'NorthWest')
    expect(api.window.startResizeDragging).toHaveBeenNthCalledWith(3, 'NorthEast')
    expect(api.window.startResizeDragging).toHaveBeenNthCalledWith(4, 'SouthWest')
    expect(api.window.startResizeDragging).toHaveBeenNthCalledWith(5, 'SouthEast')
    expect(api.window.startDragging).not.toHaveBeenCalled()
    expect(document.querySelector('.buff-overlay')).not.toHaveAttribute('data-show-border')
    expect(document.querySelector('.buff-overlay')).toHaveAttribute(
      'data-color-scheme',
      'blackWhite'
    )

    emit({
      mode: 'waiting',
      message: '等待金周天',
      items: [],
      emittedAtUnixMs: Date.now(),
      editable: false,
      colorScheme: 'blackWhite'
    })
    fireEvent.pointerDown(screen.getByText('等待金周天'), { button: 0 })

    expect(screen.queryByRole('button', { name: '调整浮窗宽度' })).not.toBeInTheDocument()
    expect(api.window.startDragging).not.toHaveBeenCalled()
  })

  it('scales content by height without changing the font size when width changes', () => {
    expect(calculateOverlayScale(30)).toBeCloseTo(30 / 92)
    expect(calculateOverlayScale(92)).toBe(1)
    expect(calculateOverlayScale(184)).toBe(2)
  })
})
