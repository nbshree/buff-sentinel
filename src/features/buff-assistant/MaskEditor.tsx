import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type PointerEvent
} from 'react'

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
  const maskRef = useRef<HTMLCanvasElement | null>(null)
  const sourceRef = useRef<HTMLImageElement | null>(null)
  const [drawing, setDrawing] = useState(false)
  const [ready, setReady] = useState(false)

  function initialize(): void {
    const canvas = visibleRef.current
    const source = sourceRef.current
    if (!canvas || !source) return
    const sourceWidth = Math.max(8, Math.round(source.naturalWidth * crop.width))
    const sourceHeight = Math.max(8, Math.round(source.naturalHeight * crop.height))
    canvas.width = sourceWidth
    canvas.height = sourceHeight
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
    const source = new Image()
    source.decoding = 'async'
    source.onload = () => {
      sourceRef.current = source
      initialize()
    }
    source.src = imageUrl
    return () => {
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

  function draw(event: PointerEvent<HTMLCanvasElement>): void {
    if (!drawing && event.type !== 'pointerdown') return
    const canvas = visibleRef.current
    const mask = maskRef.current
    if (!canvas || !mask) return
    const bounds = canvas.getBoundingClientRect()
    const x = ((event.clientX - bounds.left) / bounds.width) * canvas.width
    const y = ((event.clientY - bounds.top) / bounds.height) * canvas.height
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

  return (
    <div className="buff-mask-editor">
      <canvas
        aria-label="模板忽略区域画笔"
        className="buff-mask-editor__canvas"
        ref={visibleRef}
        onPointerDown={(event) => {
          event.currentTarget.setPointerCapture(event.pointerId)
          setDrawing(true)
          draw(event)
        }}
        onPointerMove={draw}
        onPointerUp={() => setDrawing(false)}
      />
      <div className="buff-mask-editor__footer">
        <span>{ready ? '红色区域不会参与识别' : '正在准备模板预览…'}</span>
        <button disabled={!ready} type="button" onClick={initialize}>
          清除遮罩
        </button>
      </div>
    </div>
  )
})
