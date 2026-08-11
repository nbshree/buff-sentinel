import { Maximize2, RotateCcw } from 'lucide-react'
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent
} from 'react'

import type { NormalizedRect } from '../../lib/buff-sentinel-api'
import { ZoomableEditorViewport } from './ZoomableEditorViewport'

export type MaskPoint = { x: number; y: number }
export type MaskStroke = { points: MaskPoint[]; radius: number }
export type MaskHistory = {
  past: MaskStroke[][]
  pastBaseMasks: Array<string | null>
  present: MaskStroke[]
  baseMaskDataUrl: string | null
}

export type MaskEditorHandle = {
  getMaskDataUrl: () => string | undefined
}

type MaskEditorProps = {
  imageUrl: string
  crop: NormalizedRect
  value: MaskHistory
  onChange: (history: MaskHistory) => void
  onRequestExpand?: () => void
  expanded?: boolean
}

const brushRadius = 0.08

export function createMaskHistory(baseMaskDataUrl: string | null = null): MaskHistory {
  return { past: [], pastBaseMasks: [], present: [], baseMaskDataUrl }
}

export function cloneMaskHistory(history: MaskHistory): MaskHistory {
  return {
    past: history.past.map((strokes) => strokes.map(cloneStroke)),
    pastBaseMasks: [...history.pastBaseMasks],
    present: history.present.map(cloneStroke),
    baseMaskDataUrl: history.baseMaskDataUrl
  }
}

export function appendMaskStroke(history: MaskHistory, stroke: MaskStroke): MaskHistory {
  return {
    past: [...history.past, history.present],
    pastBaseMasks: [...history.pastBaseMasks, history.baseMaskDataUrl],
    present: [...history.present, stroke],
    baseMaskDataUrl: history.baseMaskDataUrl
  }
}

export function undoMaskHistory(history: MaskHistory): MaskHistory {
  const previous = history.past[history.past.length - 1]
  if (!previous) return history
  return {
    past: history.past.slice(0, -1),
    pastBaseMasks: history.pastBaseMasks.slice(0, -1),
    present: previous,
    baseMaskDataUrl: history.pastBaseMasks[history.pastBaseMasks.length - 1] ?? null
  }
}

export function clearMaskHistory(history: MaskHistory): MaskHistory {
  if (history.present.length === 0 && !history.baseMaskDataUrl) return history
  return {
    past: [...history.past, history.present],
    pastBaseMasks: [...history.pastBaseMasks, history.baseMaskDataUrl],
    present: [],
    baseMaskDataUrl: null
  }
}

export const MaskEditor = forwardRef<MaskEditorHandle, MaskEditorProps>(function MaskEditor(
  { imageUrl, crop, value, onChange, onRequestExpand, expanded = false },
  ref
) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const sourceRef = useRef<HTMLImageElement | null>(null)
  const baseMaskRef = useRef<HTMLImageElement | null>(null)
  const activeStrokeRef = useRef<MaskStroke | null>(null)
  const pointerStartRef = useRef<{ x: number; y: number } | null>(null)
  const clickTimerRef = useRef<number | null>(null)
  const [sourceReady, setSourceReady] = useState(false)
  const [baseMaskReady, setBaseMaskReady] = useState(!value.baseMaskDataUrl)
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 })
  const [draftStroke, setDraftStroke] = useState<MaskStroke | null>(null)

  useEffect(() => {
    setSourceReady(false)
    const source = new Image()
    source.decoding = 'async'
    source.onload = () => {
      sourceRef.current = source
      const width = Math.max(8, Math.round(source.naturalWidth * crop.width))
      const height = Math.max(8, Math.round(source.naturalHeight * crop.height))
      setDimensions({ width, height })
      setSourceReady(true)
    }
    source.src = imageUrl
    return () => {
      if (clickTimerRef.current !== null) window.clearTimeout(clickTimerRef.current)
      source.onload = null
      sourceRef.current = null
    }
  }, [imageUrl, crop.x, crop.y, crop.width, crop.height])

  useEffect(() => {
    baseMaskRef.current = null
    if (!value.baseMaskDataUrl) {
      setBaseMaskReady(true)
      return
    }
    setBaseMaskReady(false)
    const mask = new Image()
    mask.decoding = 'async'
    mask.onload = () => {
      baseMaskRef.current = mask
      setBaseMaskReady(true)
    }
    mask.src = value.baseMaskDataUrl
    return () => {
      mask.onload = null
      baseMaskRef.current = null
    }
  }, [value.baseMaskDataUrl])

  const ready = sourceReady && baseMaskReady

  useEffect(() => {
    const canvas = canvasRef.current
    const source = sourceRef.current
    if (!canvas || !source || !ready) return
    canvas.width = dimensions.width
    canvas.height = dimensions.height
    renderVisibleCanvas(
      canvas,
      source,
      baseMaskRef.current,
      crop,
      value.present,
      draftStroke
    )
  }, [crop, dimensions, draftStroke, ready, value.baseMaskDataUrl, value.present])

  useImperativeHandle(
    ref,
    () => ({
      getMaskDataUrl: () => {
        if (!dimensions.width || !dimensions.height) return undefined
        return renderMaskDataUrl(
          dimensions.width,
          dimensions.height,
          baseMaskRef.current,
          value.present
        )
      }
    }),
    [crop, dimensions, value.baseMaskDataUrl, value.present]
  )

  function pointFromEvent(event: PointerEvent<HTMLCanvasElement>): MaskPoint {
    const bounds = event.currentTarget.getBoundingClientRect()
    return {
      x: clamp((event.clientX - bounds.left) / bounds.width),
      y: clamp((event.clientY - bounds.top) / bounds.height)
    }
  }

  function commitStroke(stroke: MaskStroke): void {
    onChange(appendMaskStroke(value, stroke))
  }

  function handlePointerDown(event: PointerEvent<HTMLCanvasElement>): void {
    if (event.button !== 0 || event.detail > 1) return
    event.currentTarget.setPointerCapture(event.pointerId)
    const point = pointFromEvent(event)
    activeStrokeRef.current = { points: [point], radius: brushRadius }
    pointerStartRef.current = { x: event.clientX, y: event.clientY }
    setDraftStroke(activeStrokeRef.current)
  }

  function handlePointerMove(event: PointerEvent<HTMLCanvasElement>): void {
    const stroke = activeStrokeRef.current
    const start = pointerStartRef.current
    if (!stroke || !start) return
    if (Math.hypot(event.clientX - start.x, event.clientY - start.y) < 2) return
    if (clickTimerRef.current !== null) {
      window.clearTimeout(clickTimerRef.current)
      clickTimerRef.current = null
    }
    const next = { ...stroke, points: [...stroke.points, pointFromEvent(event)] }
    activeStrokeRef.current = next
    setDraftStroke(next)
  }

  function handlePointerUp(event: PointerEvent<HTMLCanvasElement>): void {
    const stroke = activeStrokeRef.current
    const start = pointerStartRef.current
    activeStrokeRef.current = null
    pointerStartRef.current = null
    setDraftStroke(null)
    if (!stroke || !start || event.detail > 1) return
    const moved = Math.hypot(event.clientX - start.x, event.clientY - start.y) >= 2
    if (moved) {
      commitStroke(stroke)
      return
    }
    clickTimerRef.current = window.setTimeout(() => {
      commitStroke(stroke)
      clickTimerRef.current = null
    }, 220)
  }

  function undo(): void {
    if (value.past.length === 0) return
    onChange(undoMaskHistory(value))
  }

  function clear(): void {
    onChange(clearMaskHistory(value))
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 'z') return
    event.preventDefault()
    undo()
  }

  const canvas = (
    <div className="buff-mask-editor__viewport">
      <canvas
        aria-label="模板忽略区域画笔"
        className="buff-mask-editor__canvas"
        ref={canvasRef}
        onDoubleClick={(event) => {
          if (!onRequestExpand) return
          event.preventDefault()
          if (clickTimerRef.current !== null) {
            window.clearTimeout(clickTimerRef.current)
            clickTimerRef.current = null
          }
          activeStrokeRef.current = null
          pointerStartRef.current = null
          setDraftStroke(null)
          onRequestExpand()
        }}
        onPointerCancel={() => {
          activeStrokeRef.current = null
          pointerStartRef.current = null
          setDraftStroke(null)
        }}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
      />
    </div>
  )

  return (
    <div
      className="buff-mask-editor"
      data-expanded={expanded}
      tabIndex={0}
      onKeyDown={handleKeyDown}
    >
      {expanded ? (
        <ZoomableEditorViewport
          label="忽略区域缩放视口"
          resetKey={`${imageUrl}:${crop.x}:${crop.y}:${crop.width}:${crop.height}`}
        >
          {canvas}
        </ZoomableEditorViewport>
      ) : (
        canvas
      )}
      <div className="buff-mask-editor__footer">
        <span>{ready ? '红色区域不会参与识别' : '正在准备模板预览…'}</span>
        <div className="buff-mask-editor__actions">
          <button
            aria-label="撤销上一笔遮罩"
            disabled={value.past.length === 0}
            title="撤销上一笔（Ctrl+Z）"
            type="button"
            onClick={undo}
          >
            <RotateCcw aria-hidden="true" />
          </button>
          {onRequestExpand ? (
            <button type="button" onClick={onRequestExpand}>
              <Maximize2 aria-hidden="true" />
              放大涂抹
            </button>
          ) : null}
          <button
            disabled={value.present.length === 0 && !value.baseMaskDataUrl}
            type="button"
            onClick={clear}
          >
            清除遮罩
          </button>
        </div>
      </div>
    </div>
  )
})

function renderVisibleCanvas(
  canvas: HTMLCanvasElement,
  source: HTMLImageElement,
  baseMask: HTMLImageElement | null,
  crop: NormalizedRect,
  strokes: MaskStroke[],
  draft: MaskStroke | null
): void {
  const context = canvas.getContext('2d')
  if (!context) return
  context.clearRect(0, 0, canvas.width, canvas.height)
  context.drawImage(
    source,
    Math.round(source.naturalWidth * crop.x),
    Math.round(source.naturalHeight * crop.y),
    canvas.width,
    canvas.height,
    0,
    0,
    canvas.width,
    canvas.height
  )
  if (baseMask) drawBaseMask(context, baseMask)
  context.strokeStyle = 'rgb(255 74 74)'
  context.fillStyle = 'rgb(255 74 74)'
  for (const stroke of draft ? [...strokes, draft] : strokes) drawStroke(context, stroke)
}

function renderMaskDataUrl(
  width: number,
  height: number,
  baseMask: HTMLImageElement | null,
  strokes: MaskStroke[]
): string {
  const canvas = document.createElement('canvas')
  canvas.width = width
  canvas.height = height
  const context = canvas.getContext('2d')
  if (!context) return ''
  context.fillStyle = '#fff'
  context.fillRect(0, 0, width, height)
  if (baseMask) context.drawImage(baseMask, 0, 0, width, height)
  context.strokeStyle = '#000'
  context.fillStyle = '#000'
  for (const stroke of strokes) drawStroke(context, stroke)
  return canvas.toDataURL('image/png')
}

function drawBaseMask(
  context: CanvasRenderingContext2D,
  mask: HTMLImageElement
): void {
  const canvas = document.createElement('canvas')
  canvas.width = context.canvas.width
  canvas.height = context.canvas.height
  const maskContext = canvas.getContext('2d')
  if (!maskContext) return
  maskContext.drawImage(mask, 0, 0, canvas.width, canvas.height)
  const pixels = maskContext.getImageData(0, 0, canvas.width, canvas.height)
  for (let index = 0; index < pixels.data.length; index += 4) {
    const opacity = 255 - pixels.data[index]
    pixels.data[index] = 255
    pixels.data[index + 1] = 74
    pixels.data[index + 2] = 74
    pixels.data[index + 3] = opacity
  }
  maskContext.putImageData(pixels, 0, 0)
  context.drawImage(canvas, 0, 0)
}

function drawStroke(context: CanvasRenderingContext2D, stroke: MaskStroke): void {
  const scale = Math.min(context.canvas.width, context.canvas.height)
  const radius = Math.max(3, scale * stroke.radius)
  const [first, ...rest] = stroke.points
  if (!first) return
  if (rest.length === 0) {
    context.beginPath()
    context.arc(
      first.x * context.canvas.width,
      first.y * context.canvas.height,
      radius,
      0,
      Math.PI * 2
    )
    context.fill()
    return
  }
  context.lineWidth = radius * 2
  context.lineCap = 'round'
  context.lineJoin = 'round'
  context.beginPath()
  context.moveTo(first.x * context.canvas.width, first.y * context.canvas.height)
  for (const point of rest) {
    context.lineTo(point.x * context.canvas.width, point.y * context.canvas.height)
  }
  context.stroke()
}

function cloneStroke(stroke: MaskStroke): MaskStroke {
  return { ...stroke, points: stroke.points.map((point) => ({ ...point })) }
}

function clamp(value: number): number {
  return Math.min(1, Math.max(0, value))
}
