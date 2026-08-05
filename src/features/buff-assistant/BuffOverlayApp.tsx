import { useEffect, useState, type PointerEvent } from 'react'

import type { BuffOverlayState, WindowResizeDirection } from '../../lib/macro-api'

import './BuffOverlayApp.css'

const hiddenState: BuffOverlayState = {
  mode: 'hidden',
  message: '',
  expectedAtUnixMs: null,
  emittedAtUnixMs: 0,
  editable: false,
  colorScheme: 'gold'
}

const defaultOverlayWidth = 330
const defaultOverlayHeight = 92

export function calculateOverlayScale(width: number, height: number): number {
  return Math.min(width / defaultOverlayWidth, height / defaultOverlayHeight)
}

export function BuffOverlayApp() {
  const [state, setState] = useState<BuffOverlayState>(hiddenState)
  const [remainingMs, setRemainingMs] = useState(0)

  useEffect(() => {
    document.documentElement.classList.add('buff-overlay-document')
    document.body.classList.add('buff-overlay-document')
    const stop = window.api.onBuffOverlayState(setState)
    return () => {
      stop()
      document.documentElement.classList.remove('buff-overlay-document')
      document.body.classList.remove('buff-overlay-document')
    }
  }, [])

  useEffect(() => {
    const updateScale = () => {
      const scale = calculateOverlayScale(window.innerWidth, window.innerHeight)
      document.documentElement.style.setProperty('--buff-overlay-scale', String(scale))
    }
    updateScale()
    const observer = new ResizeObserver(updateScale)
    observer.observe(document.documentElement)
    return () => {
      observer.disconnect()
      document.documentElement.style.removeProperty('--buff-overlay-scale')
    }
  }, [])

  useEffect(() => {
    if (state.mode !== 'countdown' || state.expectedAtUnixMs === null) {
      setRemainingMs(0)
      return
    }
    const update = () => setRemainingMs(Math.max(0, state.expectedAtUnixMs! - Date.now()))
    update()
    const timer = window.setInterval(update, 50)
    return () => window.clearInterval(timer)
  }, [state.expectedAtUnixMs, state.mode])

  if (state.mode === 'hidden') return null

  const seconds = (remainingMs / 1000).toFixed(1)
  const warning = state.mode === 'countdown' && remainingMs <= 3_000
  const intense = state.mode === 'countdown' && remainingMs <= 1_000

  function handlePointerDown(event: PointerEvent<HTMLDivElement>): void {
    if (!state.editable || event.button !== 0) return
    void window.api.window.startDragging()
  }

  function handleResizePointerDown(
    direction: WindowResizeDirection,
    event: PointerEvent<HTMLButtonElement>
  ): void {
    event.stopPropagation()
    if (!state.editable || event.button !== 0) return
    void window.api.window.startResizeDragging(direction)
  }

  return (
    <div
      className="buff-overlay"
      data-editable={state.editable}
      data-intense={intense}
      data-mode={state.mode}
      data-color-scheme={state.colorScheme}
      data-warning={warning}
      onPointerDown={handlePointerDown}
    >
      <span className="buff-overlay__glow" />
      {state.mode === 'waiting' ? (
        <div className="buff-overlay__waiting">
          <span />
          {state.message}
        </div>
      ) : (
        <>
          <div className="buff-overlay__label">{state.message}</div>
          {state.mode === 'countdown' ? (
            <div className="buff-overlay__countdown">
              <strong>{seconds}</strong>
              <span>秒</span>
            </div>
          ) : null}
          {state.mode === 'editing' ? <small>按住拖动</small> : null}
        </>
      )}
      {state.editable ? (
        <>
          <button
            aria-label="调整浮窗宽度"
            className="buff-overlay__resize-handle buff-overlay__resize-handle--east"
            type="button"
            onPointerDown={(event) => handleResizePointerDown('East', event)}
          />
          <button
            aria-label="调整浮窗高度"
            className="buff-overlay__resize-handle buff-overlay__resize-handle--south"
            type="button"
            onPointerDown={(event) => handleResizePointerDown('South', event)}
          />
          <button
            aria-label="调整浮窗大小"
            className="buff-overlay__resize-handle buff-overlay__resize-handle--south-east"
            type="button"
            onPointerDown={(event) => handleResizePointerDown('SouthEast', event)}
          />
        </>
      ) : null}
    </div>
  )
}
