import { useEffect, useState, type PointerEvent } from 'react'

import type { BuffOverlayState, WindowResizeDirection } from '../../lib/buff-sentinel-api'

import './BuffOverlayApp.css'

const hiddenState: BuffOverlayState = {
  mode: 'hidden',
  message: '',
  items: [],
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
  const [now, setNow] = useState(Date.now())

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
    if (state.mode !== 'countdown' || state.items.length === 0) {
      return
    }
    const update = () => setNow(Date.now())
    update()
    const timer = window.setInterval(update, 50)
    return () => window.clearInterval(timer)
  }, [state.items, state.mode])

  if (state.mode === 'hidden') return null

  const minimumRemaining = state.items.reduce((minimum, item) => {
    const remaining = item.expectedAtUnixMs === null ? Number.POSITIVE_INFINITY : item.expectedAtUnixMs - now
    return Math.min(minimum, remaining)
  }, Number.POSITIVE_INFINITY)
  const warning = state.mode === 'countdown' && minimumRemaining <= 3_000
  const intense = state.mode === 'countdown' && minimumRemaining <= 1_000

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
      ) : state.items.length > 0 ? (
        <div className="buff-overlay__items">
          {state.items.map((item) => {
            const remainingMs =
              item.expectedAtUnixMs === null ? 0 : Math.max(0, item.expectedAtUnixMs - now)
            return (
              <div className="buff-overlay__item" data-mode={item.mode} key={item.listenerId}>
                <div className="buff-overlay__label">{item.name}</div>
                {item.mode === 'countdown' ? (
                  <div className="buff-overlay__countdown">
                    <strong>{(remainingMs / 1000).toFixed(1)}</strong>
                    <span>秒</span>
                  </div>
                ) : (
                  <div className="buff-overlay__confirming">等待确认</div>
                )}
              </div>
            )
          })}
        </div>
      ) : (
        <>
          <div className="buff-overlay__label">{state.message}</div>
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
        </>
      ) : null}
    </div>
  )
}
