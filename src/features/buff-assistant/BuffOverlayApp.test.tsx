import { act, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { BuffOverlayState, MacroAPI } from '@/lib/macro-api'
import { createMacroApi, installMacroApi } from '@/test/test-utils'

import { BuffOverlayApp } from './BuffOverlayApp'

let emitOverlayState: (state: BuffOverlayState) => void

function renderOverlay(): void {
  const api: MacroAPI = createMacroApi()
  api.onBuffOverlayState = vi.fn((callback) => {
    emitOverlayState = callback
    return () => undefined
  })
  installMacroApi(api)
  render(<BuffOverlayApp />)
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
      editable: false
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
      editable: false
    })
    emit({
      mode: 'countdown',
      message: '距离下一次金周天',
      expectedAtUnixMs: Date.now() + 20_000,
      emittedAtUnixMs: Date.now(),
      editable: false
    })

    expect(screen.getByText('距离下一次金周天')).toBeInTheDocument()
    expect(screen.getByText('20.0')).toBeInTheDocument()

    act(() => vi.advanceTimersByTime(500))

    expect(screen.getByText('19.5')).toBeInTheDocument()
  })
})
