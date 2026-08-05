import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { BuffOverlayState, MacroAPI } from '@/lib/macro-api'
import { createMacroApi, installMacroApi } from '@/test/test-utils'

import { BuffOverlayApp, calculateOverlayScale } from './BuffOverlayApp'

let emitOverlayState: (state: BuffOverlayState) => void

function renderOverlay(): MacroAPI {
  const api: MacroAPI = createMacroApi()
  api.onBuffOverlayState = vi.fn((callback) => {
    emitOverlayState = callback
    return () => undefined
  })
  installMacroApi(api)
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
      expectedAtUnixMs: null,
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
      expectedAtUnixMs: null,
      emittedAtUnixMs: Date.now(),
      editable: false,
      colorScheme: 'gold'
    })
    emit({
      mode: 'countdown',
      message: '距离下一次金周天',
      expectedAtUnixMs: Date.now() + 20_000,
      emittedAtUnixMs: Date.now(),
      editable: false,
      colorScheme: 'gold'
    })

    expect(screen.getByText('距离下一次金周天')).toBeInTheDocument()
    expect(screen.getByText('20.0')).toBeInTheDocument()

    act(() => vi.advanceTimersByTime(500))

    expect(screen.getByText('19.5')).toBeInTheDocument()
  })

  it('shows the simplified waiting message', () => {
    renderOverlay()

    emit({
      mode: 'waiting',
      message: '等待金周天',
      expectedAtUnixMs: null,
      emittedAtUnixMs: Date.now(),
      editable: false,
      colorScheme: 'gold'
    })

    expect(screen.getByText('等待金周天')).toBeInTheDocument()
    expect(screen.queryByText(/脱战/)).not.toBeInTheDocument()
  })

  it('only exposes resize handles while editing and keeps resize separate from dragging', () => {
    const api = renderOverlay()

    emit({
      mode: 'editing',
      message: '拖动调整位置与大小',
      expectedAtUnixMs: null,
      emittedAtUnixMs: Date.now(),
      editable: true,
      colorScheme: 'blackWhite'
    })

    fireEvent.pointerDown(screen.getByRole('button', { name: '调整浮窗宽度' }), { button: 0 })
    fireEvent.pointerDown(screen.getByRole('button', { name: '调整浮窗高度' }), { button: 0 })
    fireEvent.pointerDown(screen.getByRole('button', { name: '调整浮窗大小' }), { button: 0 })

    expect(api.window.startResizeDragging).toHaveBeenNthCalledWith(1, 'East')
    expect(api.window.startResizeDragging).toHaveBeenNthCalledWith(2, 'South')
    expect(api.window.startResizeDragging).toHaveBeenNthCalledWith(3, 'SouthEast')
    expect(api.window.startDragging).not.toHaveBeenCalled()
    expect(document.querySelector('.buff-overlay')).not.toHaveAttribute('data-show-border')
    expect(document.querySelector('.buff-overlay')).toHaveAttribute(
      'data-color-scheme',
      'blackWhite'
    )

    emit({
      mode: 'waiting',
      message: '等待金周天',
      expectedAtUnixMs: null,
      emittedAtUnixMs: Date.now(),
      editable: false,
      colorScheme: 'blackWhite'
    })
    fireEvent.pointerDown(screen.getByText('等待金周天'), { button: 0 })

    expect(screen.queryByRole('button', { name: '调整浮窗大小' })).not.toBeInTheDocument()
    expect(api.window.startDragging).not.toHaveBeenCalled()
  })

  it('scales content by the smaller dimension ratio', () => {
    expect(calculateOverlayScale(75, 30)).toBeCloseTo(75 / 330)
    expect(calculateOverlayScale(330, 92)).toBe(1)
    expect(calculateOverlayScale(800, 300)).toBeCloseTo(800 / 330)
    expect(calculateOverlayScale(660, 92)).toBe(1)
  })
})
