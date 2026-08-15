import { Palette, Pipette, RotateCcw } from 'lucide-react'
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type KeyboardEvent,
  type PointerEvent
} from 'react'

import { Button } from '../../components/ui/button'
import type { NormalizedRect } from '../../lib/buff-sentinel-api'
import { EditorExpandButton } from './EditorExpandButton'
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

const defaultBrushDiameter = 3
const maximumBrushDiameter = 48
const defaultColorTolerance = 32
const maximumColorTolerance = 128

type ColorSegmentationResult = {
  maskPixels: Uint8ClampedArray
  ignoredPercent: number
}

type PickedColorMaskResult = {
  color: [number, number, number]
  maskPixels: Uint8ClampedArray
  matchedCount: number
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
  const [brushDiameter, setBrushDiameter] = useState(defaultBrushDiameter)
  const [colorPicking, setColorPicking] = useState(false)
  const [colorTolerance, setColorTolerance] = useState(defaultColorTolerance)
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
  const shortestSide = Math.min(dimensions.width, dimensions.height)
  const brushDiameterLimit = shortestSide
    ? Math.max(1, Math.min(maximumBrushDiameter, Math.floor(shortestSide / 2)))
    : maximumBrushDiameter
  const effectiveBrushDiameter = Math.min(brushDiameter, brushDiameterLimit)

  useEffect(() => {
    setBrushDiameter((current) => Math.min(current, brushDiameterLimit))
  }, [brushDiameterLimit])

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
    const pixel = imagePixelFromClientPoint(
      event.clientX,
      event.clientY,
      bounds,
      dimensions.width,
      dimensions.height
    )
    return {
      x: (pixel.x + 0.5) / dimensions.width,
      y: (pixel.y + 0.5) / dimensions.height
    }
  }

  function commitStroke(stroke: MaskStroke): void {
    onChange(appendMaskStroke(value, stroke))
  }

  function resetActiveStroke(): void {
    activeStrokeRef.current = null
    pointerStartRef.current = null
    setDraftStroke(null)
  }

  function toggleColorPicker(): void {
    if (!expanded || !ready) return
    if (clickTimerRef.current !== null) {
      window.clearTimeout(clickTimerRef.current)
      clickTimerRef.current = null
    }
    resetActiveStroke()
    setSegmentationError('')
    setSegmentationMessage('')
    setColorPicking((current) => !current)
  }

  function applyPickedColor(point: MaskPoint): void {
    const source = sourceRef.current
    if (!source || !ready) return
    setSegmentationError('')
    setSegmentationMessage('')
    try {
      const sourcePixels = readCroppedSourcePixels(
        source,
        crop,
        dimensions.width,
        dimensions.height
      )
      const maskCanvas = renderMaskCanvas(
        dimensions.width,
        dimensions.height,
        baseMaskRef.current,
        value.present
      )
      const maskContext = maskCanvas.getContext('2d', { willReadFrequently: true })
      if (!maskContext) throw new Error('无法读取当前遮罩')
      const maskRgba = maskContext.getImageData(0, 0, dimensions.width, dimensions.height).data
      const currentMask = new Uint8ClampedArray(dimensions.width * dimensions.height)
      for (let pixel = 0; pixel < currentMask.length; pixel += 1) {
        currentMask[pixel] = maskRgba[pixel * 4]
      }
      const sampleX = Math.min(dimensions.width - 1, Math.floor(point.x * dimensions.width))
      const sampleY = Math.min(dimensions.height - 1, Math.floor(point.y * dimensions.height))
      const result = maskPixelsMatchingColor(
        sourcePixels.data,
        currentMask,
        dimensions.width,
        dimensions.height,
        sampleX,
        sampleY,
        colorTolerance
      )
      onChange(
        replaceMaskHistory(
          value,
          renderMaskPixelsDataUrl(result.maskPixels, dimensions.width, dimensions.height)
        )
      )
      const percent = Math.round((result.matchedCount * 100) / currentMask.length)
      setSegmentationMessage(
        `已按 ${formatRgbColor(result.color)} 涂抹 ${result.matchedCount} 个像素（${percent}%）`
      )
    } catch (reason) {
      setSegmentationError(reason instanceof Error ? reason.message : String(reason))
    }
  }

  function handlePointerDown(event: PointerEvent<HTMLCanvasElement>): void {
    if (!ready || event.button !== 0 || event.detail > 1) return
    if (expanded && colorPicking) {
      event.preventDefault()
      applyPickedColor(pointFromEvent(event))
      setColorPicking(false)
      return
    }
    event.currentTarget.setPointerCapture(event.pointerId)
    const point = pointFromEvent(event)
    activeStrokeRef.current = {
      points: [point],
      radius: brushRadiusFromDiameter(effectiveBrushDiameter, dimensions.width, dimensions.height)
    }
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
    resetActiveStroke()
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
        aria-label={colorPicking ? '选择要快速涂抹的颜色' : '模板忽略区域画笔'}
        className="buff-mask-editor__canvas"
        data-color-picking={colorPicking}
        ref={canvasRef}
        onDoubleClick={(event) => {
          if (!onRequestExpand) return
          event.preventDefault()
          if (clickTimerRef.current !== null) {
            window.clearTimeout(clickTimerRef.current)
            clickTimerRef.current = null
          }
          resetActiveStroke()
          onRequestExpand()
        }}
        onPointerCancel={resetActiveStroke}
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
        <span aria-live="polite">
          {segmentationError ||
            (colorPicking ? '点击图像中的一个像素，立即涂抹全图相似颜色' : '') ||
            segmentationMessage ||
            (ready ? '红色区域不会参与识别' : '正在准备模板预览…')}
        </span>
        <label className="buff-mask-editor__brush-control">
          <span>笔刷 {effectiveBrushDiameter} px</span>
          <input
            aria-label="遮罩笔刷大小"
            disabled={!ready}
            max={brushDiameterLimit}
            min="1"
            step="1"
            type="range"
            value={effectiveBrushDiameter}
            onChange={(event) => setBrushDiameter(Number(event.currentTarget.value))}
          />
          <span className="buff-mask-editor__brush-range">1–{brushDiameterLimit} px</span>
        </label>
        {expanded ? (
          <label className="buff-mask-editor__color-control">
            <span>颜色容差 {colorTolerance}</span>
            <input
              aria-label="相似颜色容差"
              disabled={!ready}
              max={maximumColorTolerance}
              min="0"
              step="1"
              type="range"
              value={colorTolerance}
              onChange={(event) => setColorTolerance(Number(event.currentTarget.value))}
            />
            <span className="buff-mask-editor__brush-range">0–{maximumColorTolerance}</span>
          </label>
        ) : null}
        <div className="buff-mask-editor__actions">
          {expanded ? (
            <Button
              aria-pressed={colorPicking}
              disabled={!ready}
              size="compact"
              title="点击后在图像中取色，并涂抹整个区域内的相似颜色"
              type="button"
              variant={colorPicking ? 'default' : 'outline'}
              onClick={toggleColorPicker}
            >
              <Pipette aria-hidden="true" />
              {colorPicking ? '取消取色' : '按颜色涂抹'}
            </Button>
          ) : null}
          <Button
            disabled={!ready}
            size="compact"
            title="按边缘背景色分离图标主体，适合纯色或近似纯色背景"
            type="button"
            variant="outline"
            onClick={generateColorMask}
          >
            <Palette aria-hidden="true" />
            颜色分割
          </Button>
          <Button
            aria-label="撤销上一笔遮罩"
            disabled={value.past.length === 0}
            size="icon-compact"
            title="撤销上一笔（Ctrl+Z）"
            type="button"
            variant="outline"
            onClick={undo}
          >
            <RotateCcw aria-hidden="true" />
          </Button>
          {onRequestExpand ? (
            <EditorExpandButton label="放大涂抹" onClick={onRequestExpand} />
          ) : null}
          <Button
            disabled={value.present.length === 0 && !value.baseMaskDataUrl}
            size="compact"
            type="button"
            variant="outline"
            onClick={clear}
          >
            清除遮罩
          </Button>
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
  const imageData = readCroppedSourcePixels(source, crop, width, height)
  const result = segmentForegroundByBorderColor(imageData.data, width, height)
  return {
    maskDataUrl: renderMaskPixelsDataUrl(result.maskPixels, width, height),
    ignoredPercent: result.ignoredPercent
  }
}

function readCroppedSourcePixels(
  source: HTMLImageElement,
  crop: NormalizedRect,
  width: number,
  height: number
): ImageData {
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
  return sourceContext.getImageData(0, 0, width, height)
}

function renderMaskPixelsDataUrl(
  maskPixels: Uint8ClampedArray,
  width: number,
  height: number
): string {
  const maskCanvas = document.createElement('canvas')
  maskCanvas.width = width
  maskCanvas.height = height
  const maskContext = maskCanvas.getContext('2d')
  if (!maskContext) throw new Error('无法创建颜色分割遮罩')
  const mask = maskContext.createImageData(width, height)
  for (let pixel = 0; pixel < maskPixels.length; pixel += 1) {
    const offset = pixel * 4
    const value = maskPixels[pixel]
    mask.data[offset] = value
    mask.data[offset + 1] = value
    mask.data[offset + 2] = value
    mask.data[offset + 3] = 255
  }
  maskContext.putImageData(mask, 0, 0)
  return maskCanvas.toDataURL('image/png')
}

export function maskPixelsMatchingColor(
  sourcePixels: Uint8ClampedArray,
  currentMask: Uint8ClampedArray,
  width: number,
  height: number,
  sampleX: number,
  sampleY: number,
  tolerance: number
): PickedColorMaskResult {
  const pixelCount = width * height
  if (
    width < 1 ||
    height < 1 ||
    sourcePixels.length !== pixelCount * 4 ||
    currentMask.length !== pixelCount
  ) {
    throw new Error('图标像素数据无效')
  }
  const x = Math.min(width - 1, Math.max(0, Math.floor(sampleX)))
  const y = Math.min(height - 1, Math.max(0, Math.floor(sampleY)))
  const sampleOffset = (y * width + x) * 4
  if (sourcePixels[sampleOffset + 3] < 32) {
    throw new Error('该像素透明，请选择可见颜色')
  }
  const color: [number, number, number] = [
    sourcePixels[sampleOffset],
    sourcePixels[sampleOffset + 1],
    sourcePixels[sampleOffset + 2]
  ]
  const threshold = Math.min(maximumColorTolerance, Math.max(0, tolerance))
  const maskPixels = new Uint8ClampedArray(currentMask)
  let matchedCount = 0
  for (let pixel = 0; pixel < pixelCount; pixel += 1) {
    const offset = pixel * 4
    if (sourcePixels[offset + 3] >= 32 && colorDistance(sourcePixels, pixel, color) <= threshold) {
      maskPixels[pixel] = 0
      matchedCount += 1
    }
  }
  return { color, maskPixels, matchedCount }
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
  return renderMaskCanvas(width, height, baseMask, strokes).toDataURL('image/png')
}

function renderMaskCanvas(
  width: number,
  height: number,
  baseMask: HTMLImageElement | null,
  strokes: MaskStroke[]
): HTMLCanvasElement {
  const canvas = document.createElement('canvas')
  canvas.width = width
  canvas.height = height
  const context = canvas.getContext('2d')
  if (!context) return canvas
  context.fillStyle = '#fff'
  context.fillRect(0, 0, width, height)
  if (baseMask) context.drawImage(baseMask, 0, 0, width, height)
  context.strokeStyle = '#000'
  context.fillStyle = '#000'
  for (const stroke of strokes) drawStroke(context, stroke)
  return canvas
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
  const radius = Math.max(0.5, scale * stroke.radius)
  const [first, ...rest] = stroke.points
  if (!first) return
  if (radius <= 0.5) {
    drawOnePixelStroke(context, stroke.points)
    return
  }
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

function drawOnePixelStroke(context: CanvasRenderingContext2D, points: MaskPoint[]): void {
  const [first, ...rest] = points
  if (!first) return
  let previous = pixelFromPoint(context.canvas, first)
  context.fillRect(previous.x, previous.y, 1, 1)
  for (const point of rest) {
    const next = pixelFromPoint(context.canvas, point)
    drawPixelLine(context, previous, next)
    previous = next
  }
}

function pixelFromPoint(canvas: HTMLCanvasElement, point: MaskPoint): { x: number; y: number } {
  return {
    x: Math.min(canvas.width - 1, Math.floor(point.x * canvas.width)),
    y: Math.min(canvas.height - 1, Math.floor(point.y * canvas.height))
  }
}

function drawPixelLine(
  context: CanvasRenderingContext2D,
  start: { x: number; y: number },
  end: { x: number; y: number }
): void {
  let x = start.x
  let y = start.y
  const deltaX = Math.abs(end.x - start.x)
  const deltaY = Math.abs(end.y - start.y)
  const stepX = start.x < end.x ? 1 : -1
  const stepY = start.y < end.y ? 1 : -1
  let error = deltaX - deltaY
  while (true) {
    context.fillRect(x, y, 1, 1)
    if (x === end.x && y === end.y) return
    const doubledError = error * 2
    if (doubledError > -deltaY) {
      error -= deltaY
      x += stepX
    }
    if (doubledError < deltaX) {
      error += deltaX
      y += stepY
    }
  }
}

export function brushRadiusFromDiameter(diameter: number, width: number, height: number): number {
  return Math.max(1, diameter) / (2 * Math.max(1, Math.min(width, height)))
}

export function imagePixelFromClientPoint(
  clientX: number,
  clientY: number,
  bounds: { left: number; top: number; width: number; height: number },
  imageWidth: number,
  imageHeight: number
): { x: number; y: number } {
  const x = Math.floor(
    clamp((clientX - bounds.left) / Math.max(1, bounds.width)) * Math.max(1, imageWidth)
  )
  const y = Math.floor(
    clamp((clientY - bounds.top) / Math.max(1, bounds.height)) * Math.max(1, imageHeight)
  )
  return {
    x: Math.min(Math.max(0, imageWidth - 1), x),
    y: Math.min(Math.max(0, imageHeight - 1), y)
  }
}

function formatRgbColor(color: [number, number, number]): string {
  return `#${color
    .map((value) => value.toString(16).padStart(2, '0'))
    .join('')
    .toUpperCase()}`
}

function cloneStroke(stroke: MaskStroke): MaskStroke {
  return { ...stroke, points: stroke.points.map((point) => ({ ...point })) }
}

function clamp(value: number): number {
  return Math.min(1, Math.max(0, value))
}
