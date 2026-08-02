import { getCurrentWindow } from '@tauri-apps/api/window'
import { useEffect, useState, type PointerEvent } from 'react'

import type { BuffOverlayState } from '../../lib/macro-api'

import './BuffOverlayApp.css'

const hiddenState: BuffOverlayState = {
  mode: 'hidden',
  message: '',
  expectedAtUnixMs: null,
  emittedAtUnixMs: 0,
  editable: false
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
    void getCurrentWindow().startDragging()
  }

  return (
    <div
      className="buff-overlay"
      data-editable={state.editable}
      data-intense={intense}
      data-mode={state.mode}
      data-warning={warning}
      onPointerDown={handlePointerDown}
    >
      <span className="buff-overlay__glow" />
      {state.mode === 'waiting' ? (
        <div className="buff-overlay__waiting">
          <span />
          等待金周天
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
    </div>
  )
}
