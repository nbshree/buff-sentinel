import { Maximize2 } from 'lucide-react'
import { useRef, useState, type PointerEvent } from 'react'

import type { NormalizedRect } from '../../lib/buff-sentinel-api'
import { ZoomableEditorViewport } from './ZoomableEditorViewport'

type Point = { x: number; y: number }
type ResizeHandle = 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w' | 'nw'
type Interaction = {
  kind: 'create' | 'move' | 'resize'
  start: Point
  startClient: Point
  initial: NormalizedRect | null
  handle?: ResizeHandle
}

type RegionSelectorProps = {
  imageUrl: string
  value: NormalizedRect | null
  label: string
  onChange: (rect: NormalizedRect) => void
  onRequestExpand?: () => void
  expanded?: boolean
}

const resizeHandles: ResizeHandle[] = ['n', 'ne', 'e', 'se', 's', 'sw', 'w', 'nw']
const minimumSize = 0.01

export function RegionSelector({
  imageUrl,
  value,
  label,
  onChange,
  onRequestExpand,
  expanded = false
}: RegionSelectorProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const interactionRef = useRef<Interaction | null>(null)
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
    if (event.button !== 0 || event.detail > 1) return
    const target = event.target as HTMLElement
    const handle = target.closest<HTMLElement>('[data-resize-handle]')?.dataset.resizeHandle as
      ResizeHandle | undefined
    const insideSelection = Boolean(target.closest('[data-region-selection]'))
    const point = pointFromEvent(event)

    event.currentTarget.setPointerCapture(event.pointerId)
    interactionRef.current = {
      kind: handle ? 'resize' : insideSelection && value ? 'move' : 'create',
      start: point,
      startClient: { x: event.clientX, y: event.clientY },
      initial: value,
      handle
    }
  }

  function handlePointerMove(event: PointerEvent<HTMLDivElement>): void {
    const interaction = interactionRef.current
    if (!interaction) return
    const point = pointFromEvent(event)
    const distance = Math.hypot(
      event.clientX - interaction.startClient.x,
      event.clientY - interaction.startClient.y
    )
    if (distance < 2) return

    if (interaction.kind === 'create') {
      setDraft(rectFromPoints(interaction.start, point))
      return
    }
    if (!interaction.initial) return
    if (interaction.kind === 'move') {
      setDraft(
        moveRect(interaction.initial, point.x - interaction.start.x, point.y - interaction.start.y)
      )
      return
    }
    if (interaction.handle) {
      setDraft(resizeRect(interaction.initial, interaction.handle, point))
    }
  }

  function finishInteraction(event: PointerEvent<HTMLDivElement>): void {
    const interaction = interactionRef.current
    if (!interaction) return
    interactionRef.current = null
    const distance = Math.hypot(
      event.clientX - interaction.startClient.x,
      event.clientY - interaction.startClient.y
    )
    const next = draft
    setDraft(null)
    if (next && distance >= (interaction.kind === 'create' ? 4 : 2)) onChange(next)
  }

  const canvas = (
    <div
      aria-label={label}
      className="buff-region-selector__canvas"
      ref={hostRef}
      role="application"
      onDoubleClick={(event) => {
        if (!onRequestExpand) return
        event.preventDefault()
        interactionRef.current = null
        setDraft(null)
        onRequestExpand()
      }}
      onPointerCancel={() => {
        interactionRef.current = null
        setDraft(null)
      }}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={finishInteraction}
    >
      <img alt="游戏窗口捕获预览" draggable={false} src={imageUrl} />
      {selection ? (
        <div
          className="buff-region-selector__selection"
          data-region-selection=""
          style={{
            left: `${selection.x * 100}%`,
            top: `${selection.y * 100}%`,
            width: `${selection.width * 100}%`,
            height: `${selection.height * 100}%`
          }}
        >
          <span className="buff-region-selector__label">{label}</span>
          {resizeHandles.map((handle) => (
            <i
              aria-hidden="true"
              className="buff-region-selector__handle"
              data-resize-handle={handle}
              key={handle}
            />
          ))}
        </div>
      ) : null}
    </div>
  )

  return (
    <div className="buff-region-selector" data-expanded={expanded}>
      {expanded ? (
        <ZoomableEditorViewport label={`${label}缩放视口`} resetKey={imageUrl}>
          {canvas}
        </ZoomableEditorViewport>
      ) : (
        canvas
      )}
      <div className="buff-region-selector__footer">
        <p>拖动空白处重选；拖动框内移动，拖动边角精调。</p>
        {onRequestExpand ? (
          <button type="button" onClick={onRequestExpand}>
            <Maximize2 aria-hidden="true" />
            放大精调
          </button>
        ) : null}
      </div>
    </div>
  )
}

function rectFromPoints(start: Point, end: Point): NormalizedRect {
  const horizontal = axisFromPoints(start.x, end.x)
  const vertical = axisFromPoints(start.y, end.y)
  return normalizeRect({
    x: horizontal.start,
    y: vertical.start,
    width: horizontal.size,
    height: vertical.size
  })
}

function axisFromPoints(start: number, end: number): { start: number; size: number } {
  const size = Math.abs(start - end)
  if (size >= minimumSize) return { start: Math.min(start, end), size }
  if (end >= start) return { start: Math.min(start, 1 - minimumSize), size: minimumSize }
  return { start: Math.max(0, start - minimumSize), size: minimumSize }
}

function moveRect(rect: NormalizedRect, dx: number, dy: number): NormalizedRect {
  return normalizeRect({
    ...rect,
    x: clamp(rect.x + dx, 0, 1 - rect.width),
    y: clamp(rect.y + dy, 0, 1 - rect.height)
  })
}

function resizeRect(rect: NormalizedRect, handle: ResizeHandle, point: Point): NormalizedRect {
  let left = rect.x
  let top = rect.y
  let right = rect.x + rect.width
  let bottom = rect.y + rect.height

  if (handle.includes('w')) left = clamp(point.x, 0, right - minimumSize)
  if (handle.includes('e')) right = clamp(point.x, left + minimumSize, 1)
  if (handle.includes('n')) top = clamp(point.y, 0, bottom - minimumSize)
  if (handle.includes('s')) bottom = clamp(point.y, top + minimumSize, 1)

  return normalizeRect({ x: left, y: top, width: right - left, height: bottom - top })
}

function normalizeRect(rect: NormalizedRect): NormalizedRect {
  return {
    x: round(rect.x),
    y: round(rect.y),
    width: round(rect.width),
    height: round(rect.height)
  }
}

function round(value: number): number {
  return Math.round(value * 1_000_000) / 1_000_000
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value))
}
