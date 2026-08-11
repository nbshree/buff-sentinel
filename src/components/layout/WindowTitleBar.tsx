import { useCallback, useEffect, useState, type MouseEvent } from 'react'
import { MousePointerClick } from 'lucide-react'

import type { WindowResizeDirection } from '../../lib/buff-sentinel-api'
import './WindowTitleBar.css'

const resizeDirections: WindowResizeDirection[] = [
  'North',
  'NorthEast',
  'East',
  'SouthEast',
  'South',
  'SouthWest',
  'West',
  'NorthWest'
]

type WindowTitleBarProps = {
  title?: string
  className?: string
}

function reportWindowError(action: string, error: unknown) {
  console.error(`${action}失败`, error)
}

export function WindowTitleBar({ title = 'Buff 哨兵', className = '' }: WindowTitleBarProps) {
  const [isMaximized, setIsMaximized] = useState(false)

  const refreshMaximizedState = useCallback(async () => {
    try {
      setIsMaximized(await window.api.window.isMaximized())
    } catch (error: unknown) {
      reportWindowError('读取窗口状态', error)
    }
  }, [])

  useEffect(() => {
    let active = true

    const refresh = async () => {
      try {
        const nextValue = await window.api.window.isMaximized()
        if (active) setIsMaximized(nextValue)
      } catch (error: unknown) {
        if (active) reportWindowError('读取窗口状态', error)
      }
    }

    void refresh()
    const unsubscribe = window.api.window.onResized(() => {
      void refresh()
    })

    return () => {
      active = false
      unsubscribe()
    }
  }, [])

  const handleDragStart = (event: MouseEvent<HTMLDivElement>) => {
    if (event.button !== 0 || event.detail > 1) return

    event.preventDefault()
    void window.api.window.startDragging().catch((error: unknown) => {
      reportWindowError('拖动窗口', error)
    })
  }

  const handleToggleMaximize = () => {
    void window.api.window
      .toggleMaximize()
      .then(refreshMaximizedState)
      .catch((error: unknown) => {
        reportWindowError('切换窗口大小', error)
      })
  }

  const handleResizeStart = (
    event: MouseEvent<HTMLDivElement>,
    direction: WindowResizeDirection
  ) => {
    if (event.button !== 0) return

    event.preventDefault()
    event.stopPropagation()
    void window.api.window.startResizeDragging(direction).catch((error: unknown) => {
      reportWindowError('调整窗口大小', error)
    })
  }

  return (
    <header className={`window-title-bar ${className}`.trim()} role="banner">
      <div
        className="window-title-bar__drag-region"
        onDoubleClick={handleToggleMaximize}
        onMouseDown={handleDragStart}
      >
        <MousePointerClick aria-hidden="true" className="window-title-bar__app-icon" size={18} />
        <span className="window-title-bar__title">{title}</span>
      </div>

      <div className="window-title-bar__controls" aria-label="窗口控制" role="group">
        <button
          aria-label="最小化窗口"
          className="window-title-bar__control"
          onClick={() => {
            void window.api.window.minimize().catch((error: unknown) => {
              reportWindowError('最小化窗口', error)
            })
          }}
          title="最小化"
          type="button"
        >
          <svg aria-hidden="true" viewBox="0 0 24 24">
            <path d="M5 12.5h14" />
          </svg>
        </button>

        <button
          aria-label={isMaximized ? '还原窗口' : '最大化窗口'}
          aria-pressed={isMaximized}
          className="window-title-bar__control"
          onClick={handleToggleMaximize}
          title={isMaximized ? '还原' : '最大化'}
          type="button"
        >
          {isMaximized ? (
            <svg aria-hidden="true" viewBox="0 0 24 24">
              <path d="M8.5 8.5h9v9h-9z" />
              <path d="M6.5 15.5h-1v-9h9v1" />
            </svg>
          ) : (
            <svg aria-hidden="true" viewBox="0 0 24 24">
              <path d="M6.5 6.5h11v11h-11z" />
            </svg>
          )}
        </button>

        <button
          aria-label="关闭窗口"
          className="window-title-bar__control window-title-bar__control--close"
          onClick={() => {
            void window.api.window.close().catch((error: unknown) => {
              reportWindowError('关闭窗口', error)
            })
          }}
          title="关闭"
          type="button"
        >
          <svg aria-hidden="true" viewBox="0 0 24 24">
            <path d="m7 7 10 10M17 7 7 17" />
          </svg>
        </button>
      </div>

      {!isMaximized
        ? resizeDirections.map((direction) => (
            <div
              aria-hidden="true"
              className={`window-resize-handle window-resize-handle--${direction.toLowerCase()}`}
              key={direction}
              onMouseDown={(event) => handleResizeStart(event, direction)}
            />
          ))
        : null}
    </header>
  )
}

export default WindowTitleBar
