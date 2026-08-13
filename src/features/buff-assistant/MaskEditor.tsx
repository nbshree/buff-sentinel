import { Maximize2, Palette, RotateCcw } from 'lucide-react'
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

type ColorSegmentationResult = {
  maskPixels: Uint8ClampedArray
  ignoredPercent: number
}

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

export function replaceMaskHistory(history: MaskHistory, maskDataUrl: string): MaskHistory {
  return {
    past: [...history.past, history.present],
    pastBaseMasks: [...history.pastBaseMasks, history.baseMaskDataUrl],
    present: [],
    baseMaskDataUrl: maskDataUrl
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
  const [baseMaskRevision, setBaseMaskRevision] = useState(0)
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 })
  const [draftStroke, setDraftStroke] = useState<MaskStroke | null>(null)
  const [segmentationError, setSegmentationError] = useState('')
  const [segmentationMessage, setSegmentationMessage] = useState('')

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
      setBaseMaskRevision((revision) => revision + 1)
    }
    mask.onerror = () => {
      baseMaskRef.current = null
      setBaseMaskReady(true)
      setSegmentationError('遮罩加载失败，请重试。')
    }
    mask.src = value.baseMaskDataUrl
    return () => {
      mask.onload = null
      mask.onerror = null
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
    renderVisibleCanvas(canvas, source, baseMaskRef.current, crop, value.present, draftStroke)
  }, [baseMaskRevision, crop, dimensions, draftStroke, ready, value.baseMaskDataUrl, value.present])

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

  function generateColorMask(): void {
    const source = sourceRef.current
    if (!source || !ready) return
    setSegmentationError('')
    setSegmentationMessage('')
    try {
      const result = renderColorBoundaryMask(source, crop, dimensions.width, dimensions.height)
      onChange(replaceMaskHistory(value, result.maskDataUrl))
      setSegmentationMessage(`颜色分割已应用，忽略 ${result.ignoredPercent}% 区域`)
    } catch (reason) {
      setSegmentationError(reason instanceof Error ? reason.message : String(reason))
    }
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
        <span>
          {segmentationError ||
            segmentationMessage ||
            (ready ? '红色区域不会参与识别' : '正在准备模板预览…')}
        </span>
        <div className="buff-mask-editor__actions">
          <button
            disabled={!ready}
            title="按边缘背景色分离图标主体，适合纯色或近似纯色背景"
            type="button"
            onClick={generateColorMask}
          >
            <Palette aria-hidden="true" />
            颜色分割
          </button>
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
            <button className="buff-mask-editor__expand" type="button" onClick={onRequestExpand}>
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

function renderColorBoundaryMask(
  source: HTMLImageElement,
  crop: NormalizedRect,
  width: number,
  height: number
): { maskDataUrl: string; ignoredPercent: number } {
  const sourceCanvas = document.createElement('canvas')
  sourceCanvas.width = width
  sourceCanvas.height = height
  const sourceContext = sourceCanvas.getContext('2d', { willReadFrequently: true })
  if (!sourceContext) throw new Error('无法读取图标颜色')
  sourceContext.drawImage(
    source,
    Math.round(source.naturalWidth * crop.x),
    Math.round(source.naturalHeight * crop.y),
    width,
    height,
    0,
    0,
    width,
    height
  )
  const imageData = sourceContext.getImageData(0, 0, width, height)
  const result = segmentForegroundByBorderColor(imageData.data, width, height)
  const maskCanvas = document.createElement('canvas')
  maskCanvas.width = width
  maskCanvas.height = height
  const maskContext = maskCanvas.getContext('2d')
  if (!maskContext) throw new Error('无法创建颜色分割遮罩')
  const mask = maskContext.createImageData(width, height)
  for (let pixel = 0; pixel < result.maskPixels.length; pixel += 1) {
    const offset = pixel * 4
    const value = result.maskPixels[pixel]
    mask.data[offset] = value
    mask.data[offset + 1] = value
    mask.data[offset + 2] = value
    mask.data[offset + 3] = 255
  }
  maskContext.putImageData(mask, 0, 0)
  return { maskDataUrl: maskCanvas.toDataURL('image/png'), ignoredPercent: result.ignoredPercent }
}

export function segmentForegroundByBorderColor(
  pixels: Uint8ClampedArray,
  width: number,
  height: number
): ColorSegmentationResult {
  if (width < 2 || height < 2 || pixels.length !== width * height * 4) {
    throw new Error('图标像素数据无效')
  }
  const borderIndexes: number[] = []
  for (let x = 0; x < width; x += 1) {
    borderIndexes.push(x, (height - 1) * width + x)
  }
  for (let y = 1; y < height - 1; y += 1) {
    borderIndexes.push(y * width, y * width + width - 1)
  }
  const buckets = new Map<number, number[]>()
  for (const pixel of borderIndexes) {
    const offset = pixel * 4
    const key =
      (Math.round(pixels[offset] / 16) << 8) |
      (Math.round(pixels[offset + 1] / 16) << 4) |
      Math.round(pixels[offset + 2] / 16)
    const bucket = buckets.get(key)
    if (bucket) bucket.push(pixel)
    else buckets.set(key, [pixel])
  }
  const dominant = [...buckets.values()].sort((left, right) => right.length - left.length)[0]
  const background = dominant.reduce(
    (sum, pixel) => {
      const offset = pixel * 4
      sum[0] += pixels[offset]
      sum[1] += pixels[offset + 1]
      sum[2] += pixels[offset + 2]
      return sum
    },
    [0, 0, 0]
  )
  background[0] /= dominant.length
  background[1] /= dominant.length
  background[2] /= dominant.length
  const borderDistances = borderIndexes
    .map((pixel) => colorDistance(pixels, pixel, background))
    .sort((left, right) => left - right)
  const typicalBorderDistance = borderDistances[Math.floor(borderDistances.length * 0.8)] ?? 0
  const threshold = Math.min(72, Math.max(28, typicalBorderDistance + 20))
  const ignored = new Uint8Array(width * height)
  const queue = [...new Set(borderIndexes)].filter(
    (pixel) => pixels[pixel * 4 + 3] < 32 || colorDistance(pixels, pixel, background) <= threshold
  )
  for (const pixel of queue) ignored[pixel] = 1
  for (let cursor = 0; cursor < queue.length; cursor += 1) {
    const pixel = queue[cursor]
    const x = pixel % width
    const y = Math.floor(pixel / width)
    const neighbors = [
      x > 0 ? pixel - 1 : -1,
      x + 1 < width ? pixel + 1 : -1,
      y > 0 ? pixel - width : -1,
      y + 1 < height ? pixel + width : -1
    ]
    for (const neighbor of neighbors) {
      if (
        neighbor >= 0 &&
        !ignored[neighbor] &&
        (pixels[neighbor * 4 + 3] < 32 || colorDistance(pixels, neighbor, background) <= threshold)
      ) {
        ignored[neighbor] = 1
        queue.push(neighbor)
      }
    }
  }
  const foreground = largestForegroundComponent(ignored, width, height)
  const maskPixels = new Uint8ClampedArray(width * height)
  let ignoredCount = 0
  for (let pixel = 0; pixel < maskPixels.length; pixel += 1) {
    maskPixels[pixel] = foreground[pixel] ? 255 : 0
    if (!foreground[pixel]) ignoredCount += 1
  }
  if (ignoredCount === 0 || ignoredCount === maskPixels.length) {
    throw new Error('颜色分割未找到可靠主体，请改用手工涂抹。')
  }
  return {
    maskPixels,
    ignoredPercent: Math.round((ignoredCount * 100) / maskPixels.length)
  }
}

function largestForegroundComponent(
  ignored: Uint8Array,
  width: number,
  height: number
): Uint8Array {
  const visited = new Uint8Array(ignored.length)
  let largest: number[] = []
  for (let start = 0; start < ignored.length; start += 1) {
    if (ignored[start] || visited[start]) continue
    const component = [start]
    visited[start] = 1
    for (let cursor = 0; cursor < component.length; cursor += 1) {
      const pixel = component[cursor]
      const x = pixel % width
      const y = Math.floor(pixel / width)
      const neighbors = [
        x > 0 ? pixel - 1 : -1,
        x + 1 < width ? pixel + 1 : -1,
        y > 0 ? pixel - width : -1,
        y + 1 < height ? pixel + width : -1
      ]
      for (const neighbor of neighbors) {
        if (neighbor >= 0 && !ignored[neighbor] && !visited[neighbor]) {
          visited[neighbor] = 1
          component.push(neighbor)
        }
      }
    }
    if (component.length > largest.length) largest = component
  }
  const foreground = new Uint8Array(ignored.length)
  for (const pixel of largest) foreground[pixel] = 1
  return foreground
}

function colorDistance(pixels: Uint8ClampedArray, pixel: number, color: number[]): number {
  const offset = pixel * 4
  return Math.hypot(
    pixels[offset] - color[0],
    pixels[offset + 1] - color[1],
    pixels[offset + 2] - color[2]
  )
}

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

function drawBaseMask(context: CanvasRenderingContext2D, mask: HTMLImageElement): void {
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
