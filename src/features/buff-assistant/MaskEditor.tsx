import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState
} from 'react'
import { ZoomIn, ZoomOut } from 'lucide-react'

import type { NormalizedRect } from '../../lib/macro-api'

export type MaskEditorHandle = {
  getMaskDataUrl: () => string | undefined
  reset: () => void
}

type MaskEditorProps = {
  imageUrl: string
  crop: NormalizedRect
}

export const MaskEditor = forwardRef<MaskEditorHandle, MaskEditorProps>(function MaskEditor(
  { imageUrl, crop },
  ref
) {
  const visibleRef = useRef<HTMLCanvasElement>(null)
  const viewportRef = useRef<HTMLDivElement>(null)
  const maskRef = useRef<HTMLCanvasElement | null>(null)
  const sourceRef = useRef<HTMLImageElement | null>(null)
  const drawingRef = useRef(false)
  const movedRef = useRef(false)
  const clickTimerRef = useRef<number | null>(null)
  const [ready, setReady] = useState(false)
  const [zoomed, setZoomed] = useState(false)
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 })

  function initialize(): void {
    const canvas = visibleRef.current
    const source = sourceRef.current
    if (!canvas || !source) return
    const sourceWidth = Math.max(8, Math.round(source.naturalWidth * crop.width))
    const sourceHeight = Math.max(8, Math.round(source.naturalHeight * crop.height))
    canvas.width = sourceWidth
    canvas.height = sourceHeight
    setDimensions({ width: sourceWidth, height: sourceHeight })
    const context = canvas.getContext('2d')
    if (!context) return
    context.clearRect(0, 0, sourceWidth, sourceHeight)
    context.drawImage(
      source,
      Math.round(source.naturalWidth * crop.x),
      Math.round(source.naturalHeight * crop.y),
      sourceWidth,
      sourceHeight,
      0,
      0,
      sourceWidth,
      sourceHeight
    )
    const mask = document.createElement('canvas')
    mask.width = sourceWidth
    mask.height = sourceHeight
    const maskContext = mask.getContext('2d')
    if (!maskContext) return
    maskContext.fillStyle = '#fff'
    maskContext.fillRect(0, 0, sourceWidth, sourceHeight)
    maskRef.current = mask
    setReady(true)
  }

  useEffect(() => {
    setReady(false)
    setZoomed(false)
    const source = new Image()
    source.decoding = 'async'
    source.onload = () => {
      sourceRef.current = source
      initialize()
    }
    source.src = imageUrl
    return () => {
      if (clickTimerRef.current !== null) window.clearTimeout(clickTimerRef.current)
      source.onload = null
      sourceRef.current = null
    }
  }, [imageUrl, crop.x, crop.y, crop.width, crop.height])

  useImperativeHandle(
    ref,
    () => ({
      getMaskDataUrl: () => maskRef.current?.toDataURL('image/png'),
      reset: initialize
    }),
    [crop, imageUrl]
  )

  function drawAt(clientX: number, clientY: number): void {
    const canvas = visibleRef.current
    const mask = maskRef.current
    if (!canvas || !mask) return
    const bounds = canvas.getBoundingClientRect()
    const x = ((clientX - bounds.left) / bounds.width) * canvas.width
    const y = ((clientY - bounds.top) / bounds.height) * canvas.height
    const radius = Math.max(3, Math.min(canvas.width, canvas.height) * 0.08)
    const maskContext = mask.getContext('2d')
    const visibleContext = canvas.getContext('2d')
    if (!maskContext || !visibleContext) return
    maskContext.fillStyle = '#000'
    maskContext.beginPath()
    maskContext.arc(x, y, radius, 0, Math.PI * 2)
    maskContext.fill()
    visibleContext.fillStyle = 'rgb(255 74 74 / 55%)'
    visibleContext.beginPath()
    visibleContext.arc(x, y, radius, 0, Math.PI * 2)
    visibleContext.fill()
  }

  function toggleZoom(origin?: { x: number; y: number }): void {
    const next = !zoomed
    setZoomed(next)
    window.requestAnimationFrame(() => {
      const viewport = viewportRef.current
      const canvas = visibleRef.current
      if (!viewport || !canvas) return
      if (!next) {
        viewport.scrollTo({ left: 0, top: 0 })
        return
      }
      const target = origin ?? { x: 0.5, y: 0.5 }
      viewport.scrollTo({
        left: target.x * canvas.clientWidth - viewport.clientWidth / 2,
        top: target.y * canvas.clientHeight - viewport.clientHeight / 2
      })
    })
  }

  const zoomScale = Math.max(
    4,
    Math.min(12, 280 / Math.max(1, Math.min(dimensions.width, dimensions.height)))
  )

  return (
    <div className="buff-mask-editor" data-zoomed={zoomed}>
      <div className="buff-mask-editor__viewport" data-zoomed={zoomed} ref={viewportRef}>
        <canvas
          aria-label="模板忽略区域画笔"
          className="buff-mask-editor__canvas"
          ref={visibleRef}
          style={
            zoomed
              ? {
                  width: dimensions.width * zoomScale,
                  height: dimensions.height * zoomScale,
                  maxWidth: 'none',
                  maxHeight: 'none'
                }
              : undefined
          }
          onDoubleClick={(event) => {
            event.preventDefault()
            if (clickTimerRef.current !== null) {
              window.clearTimeout(clickTimerRef.current)
              clickTimerRef.current = null
            }
            const bounds = event.currentTarget.getBoundingClientRect()
            toggleZoom({
              x: (event.clientX - bounds.left) / bounds.width,
              y: (event.clientY - bounds.top) / bounds.height
            })
          }}
          onPointerDown={(event) => {
            event.currentTarget.setPointerCapture(event.pointerId)
            drawingRef.current = true
            movedRef.current = false
          }}
          onPointerMove={(event) => {
            if (!drawingRef.current) return
            movedRef.current = true
            if (clickTimerRef.current !== null) {
              window.clearTimeout(clickTimerRef.current)
              clickTimerRef.current = null
            }
            drawAt(event.clientX, event.clientY)
          }}
          onPointerUp={(event) => {
            drawingRef.current = false
            if (movedRef.current || event.detail > 1) return
            const { clientX, clientY } = event
            clickTimerRef.current = window.setTimeout(() => {
              drawAt(clientX, clientY)
              clickTimerRef.current = null
            }, 220)
          }}
          onPointerCancel={() => {
            drawingRef.current = false
          }}
        />
      </div>
      <div className="buff-mask-editor__footer">
        <span>{ready ? '红色区域不会参与识别' : '正在准备模板预览…'}</span>
        <div className="buff-mask-editor__actions">
          <button
            aria-label={zoomed ? '缩小遮罩编辑区' : '放大遮罩编辑区'}
            disabled={!ready}
            title={zoomed ? '恢复适配大小' : '放大涂抹'}
            type="button"
            onClick={() => toggleZoom()}
          >
            {zoomed ? <ZoomOut aria-hidden="true" /> : <ZoomIn aria-hidden="true" />}
          </button>
          <button disabled={!ready} type="button" onClick={initialize}>
            清除遮罩
          </button>
        </div>
      </div>
    </div>
  )
})
