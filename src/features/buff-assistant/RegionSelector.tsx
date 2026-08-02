import { useRef, useState, type PointerEvent } from 'react'

import type { NormalizedRect } from '../../lib/macro-api'

type Point = { x: number; y: number }

type RegionSelectorProps = {
  imageUrl: string
  value: NormalizedRect | null
  label: string
  onChange: (rect: NormalizedRect) => void
}

export function RegionSelector({ imageUrl, value, label, onChange }: RegionSelectorProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const [start, setStart] = useState<Point | null>(null)
  const [draft, setDraft] = useState<NormalizedRect | null>(null)
  const selection = draft ?? value

  function pointFromEvent(event: PointerEvent<HTMLDivElement>): Point {
    const bounds = hostRef.current?.getBoundingClientRect()
    if (!bounds) return { x: 0, y: 0 }
    return {
      x: clamp((event.clientX - bounds.left) / bounds.width, 0, 1),
      y: clamp((event.clientY - bounds.top) / bounds.height, 0, 1)
    }
  }

  function handlePointerDown(event: PointerEvent<HTMLDivElement>): void {
    event.currentTarget.setPointerCapture(event.pointerId)
    const point = pointFromEvent(event)
    setStart(point)
    setDraft({ x: point.x, y: point.y, width: 0.01, height: 0.01 })
  }

  function handlePointerMove(event: PointerEvent<HTMLDivElement>): void {
    if (!start) return
    setDraft(rectFromPoints(start, pointFromEvent(event)))
  }

  function handlePointerUp(event: PointerEvent<HTMLDivElement>): void {
    if (!start) return
    const rect = rectFromPoints(start, pointFromEvent(event))
    setStart(null)
    setDraft(null)
    if (rect.width >= 0.01 && rect.height >= 0.01) onChange(rect)
  }

  return (
    <div className="buff-region-selector">
      <div
        aria-label={label}
        className="buff-region-selector__canvas"
        ref={hostRef}
        role="application"
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
      >
        <img alt="游戏窗口采集预览" draggable={false} src={imageUrl} />
        {selection ? (
          <div
            className="buff-region-selector__selection"
            style={{
              left: `${selection.x * 100}%`,
              top: `${selection.y * 100}%`,
              width: `${selection.width * 100}%`,
              height: `${selection.height * 100}%`
            }}
          >
            <span>{label}</span>
          </div>
        ) : null}
      </div>
      <p>按住鼠标拖动框选；再次拖动可重选。</p>
    </div>
  )
}

function rectFromPoints(start: Point, end: Point): NormalizedRect {
  return {
    x: Math.min(start.x, end.x),
    y: Math.min(start.y, end.y),
    width: Math.max(0.01, Math.abs(start.x - end.x)),
    height: Math.max(0.01, Math.abs(start.y - end.y))
  }
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value))
}
