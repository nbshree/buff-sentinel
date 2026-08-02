import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode
} from 'react'

type ZoomableEditorViewportProps = {
  children: ReactNode
  label: string
  resetKey: string
}

type Dimensions = { width: number; height: number }
type ZoomAnchor = { clientX: number; clientY: number; x: number; y: number }

const minimumZoom = 1
const maximumZoom = 8

export function ZoomableEditorViewport({ children, label, resetKey }: ZoomableEditorViewportProps) {
  const viewportRef = useRef<HTMLDivElement>(null)
  const contentRef = useRef<HTMLDivElement>(null)
  const pendingAnchorRef = useRef<ZoomAnchor | null>(null)
  const [zoom, setZoom] = useState(minimumZoom)
  const [dimensions, setDimensions] = useState<Dimensions>({ width: 0, height: 0 })

  useLayoutEffect(() => {
    const content = contentRef.current
    if (!content) return

    function updateDimensions(width: number, height: number): void {
      if (width <= 0 || height <= 0) return
      setDimensions((current) =>
        current.width === width && current.height === height ? current : { width, height }
      )
    }

    const bounds = content.getBoundingClientRect()
    updateDimensions(bounds.width / zoom, bounds.height / zoom)
    const observer = new ResizeObserver(([entry]) => {
      if (entry) updateDimensions(entry.contentRect.width, entry.contentRect.height)
    })
    observer.observe(content)
    return () => observer.disconnect()
  }, [zoom])

  useLayoutEffect(() => {
    const anchor = pendingAnchorRef.current
    const viewport = viewportRef.current
    const content = contentRef.current
    if (!anchor || !viewport || !content) return
    pendingAnchorRef.current = null
    const bounds = content.getBoundingClientRect()
    viewport.scrollLeft += bounds.left + anchor.x * bounds.width - anchor.clientX
    viewport.scrollTop += bounds.top + anchor.y * bounds.height - anchor.clientY
  }, [zoom])

  useEffect(() => {
    pendingAnchorRef.current = null
    setZoom(minimumZoom)
    const viewport = viewportRef.current
    if (viewport) {
      viewport.scrollLeft = 0
      viewport.scrollTop = 0
    }
  }, [resetKey])

  useEffect(() => {
    const viewport = viewportRef.current
    if (!viewport) return
    viewport.addEventListener('wheel', handleWheel, { passive: false })
    return () => viewport.removeEventListener('wheel', handleWheel)
  }, [zoom])

  function handleWheel(event: WheelEvent): void {
    const content = contentRef.current
    if (!content) return
    event.preventDefault()
    const nextZoom = calculateWheelZoom(zoom, event.deltaY)
    if (nextZoom === zoom) return
    const bounds = content.getBoundingClientRect()
    pendingAnchorRef.current = {
      clientX: event.clientX,
      clientY: event.clientY,
      x: clamp((event.clientX - bounds.left) / Math.max(1, bounds.width), 0, 1),
      y: clamp((event.clientY - bounds.top) / Math.max(1, bounds.height), 0, 1)
    }
    setZoom(nextZoom)
  }

  function resetZoom(): void {
    pendingAnchorRef.current = null
    setZoom(minimumZoom)
    const viewport = viewportRef.current
    if (viewport) {
      viewport.scrollLeft = 0
      viewport.scrollTop = 0
    }
  }

  return (
    <div className="buff-editor-zoom" aria-label={label}>
      <div className="buff-editor-zoom__viewport" data-zoom-viewport="" ref={viewportRef}>
        <div
          className="buff-editor-zoom__stage"
          style={
            dimensions.width > 0
              ? { width: dimensions.width * zoom, height: dimensions.height * zoom }
              : undefined
          }
        >
          <div
            className="buff-editor-zoom__content"
            data-zoom-content=""
            ref={contentRef}
            style={
              {
                '--editor-zoom': zoom,
                '--editor-zoom-inverse': 1 / zoom,
                transform: `scale(${zoom})`
              } as CSSProperties
            }
          >
            {children}
          </div>
        </div>
      </div>
      <div className="buff-editor-zoom__toolbar">
        <span>滚轮缩放 · {Math.round(zoom * 100)}%</span>
        <button disabled={zoom === minimumZoom} type="button" onClick={resetZoom}>
          重置为 100%
        </button>
      </div>
    </div>
  )
}

export function calculateWheelZoom(current: number, deltaY: number): number {
  if (deltaY === 0) return current
  return round(clamp(current * Math.exp(-deltaY * 0.0015), minimumZoom, maximumZoom))
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value))
}

function round(value: number): number {
  return Math.round(value * 10_000) / 10_000
}
