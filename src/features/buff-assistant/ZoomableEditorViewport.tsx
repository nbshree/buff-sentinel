import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
  type ReactNode
} from 'react'

import { Button } from '../../components/ui/button'

type ZoomableEditorViewportProps = {
  children: ReactNode
  label: string
  resetKey: string
}

type Dimensions = { width: number; height: number }
type ZoomAnchor = { clientX: number; clientY: number; x: number; y: number }
type PanGesture = {
  pointerId: number
  startX: number
  startY: number
  scrollLeft: number
  scrollTop: number
}

const minimumZoom = 1
const maximumZoom = 8

export function ZoomableEditorViewport({ children, label, resetKey }: ZoomableEditorViewportProps) {
  const viewportRef = useRef<HTMLDivElement>(null)
  const contentRef = useRef<HTMLDivElement>(null)
  const pendingAnchorRef = useRef<ZoomAnchor | null>(null)
  const panGestureRef = useRef<PanGesture | null>(null)
  const [zoom, setZoom] = useState(minimumZoom)
  const [dimensions, setDimensions] = useState<Dimensions>({ width: 0, height: 0 })
  const [viewportDimensions, setViewportDimensions] = useState<Dimensions>({ width: 0, height: 0 })
  const [panning, setPanning] = useState(false)

  useLayoutEffect(() => {
    const content = contentRef.current
    const viewport = viewportRef.current
    if (!content || !viewport) return

    function updateContentDimensions(width: number, height: number): void {
      if (width <= 0 || height <= 0) return
      setDimensions((current) =>
        current.width === width && current.height === height ? current : { width, height }
      )
    }

    function updateViewportDimensions(width: number, height: number): void {
      if (width <= 0 || height <= 0) return
      setViewportDimensions((current) =>
        current.width === width && current.height === height ? current : { width, height }
      )
    }

    const contentBounds = content.getBoundingClientRect()
    updateContentDimensions(contentBounds.width, contentBounds.height)
    const viewportBounds = getViewportDimensions(viewport)
    updateViewportDimensions(viewportBounds.width, viewportBounds.height)

    const contentObserver = new ResizeObserver(([entry]) => {
      if (entry) updateContentDimensions(entry.contentRect.width, entry.contentRect.height)
    })
    const viewportObserver = new ResizeObserver(([entry]) => {
      if (entry) {
        const viewportBounds = getViewportDimensions(viewport, entry)
        updateViewportDimensions(viewportBounds.width, viewportBounds.height)
      }
    })
    contentObserver.observe(content)
    viewportObserver.observe(viewport)
    return () => {
      contentObserver.disconnect()
      viewportObserver.disconnect()
    }
  }, [])

  const fitScale = calculateFitScale(dimensions, viewportDimensions)
  const displayScale = fitScale * zoom
  const scaledWidth = dimensions.width * displayScale
  const scaledHeight = dimensions.height * displayScale
  const stageWidth = Math.max(viewportDimensions.width, scaledWidth)
  const stageHeight = Math.max(viewportDimensions.height, scaledHeight)
  const contentLeft = Math.max(0, (stageWidth - scaledWidth) / 2)
  const contentTop = Math.max(0, (stageHeight - scaledHeight) / 2)

  useLayoutEffect(() => {
    const anchor = pendingAnchorRef.current
    const viewport = viewportRef.current
    const content = contentRef.current
    if (!anchor || !viewport || !content) return
    pendingAnchorRef.current = null
    const bounds = content.getBoundingClientRect()
    viewport.scrollLeft += bounds.left + anchor.x * bounds.width - anchor.clientX
    viewport.scrollTop += bounds.top + anchor.y * bounds.height - anchor.clientY
  }, [displayScale])

  useEffect(() => {
    pendingAnchorRef.current = null
    panGestureRef.current = null
    setPanning(false)
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

  function handlePanStart(event: ReactPointerEvent<HTMLDivElement>): void {
    if (!event.ctrlKey || event.button !== 0) return
    event.preventDefault()
    event.stopPropagation()
    event.currentTarget.setPointerCapture(event.pointerId)
    panGestureRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      scrollLeft: event.currentTarget.scrollLeft,
      scrollTop: event.currentTarget.scrollTop
    }
    setPanning(true)
  }

  function handlePanMove(event: ReactPointerEvent<HTMLDivElement>): void {
    const gesture = panGestureRef.current
    if (!gesture || gesture.pointerId !== event.pointerId) return
    event.preventDefault()
    event.stopPropagation()
    event.currentTarget.scrollLeft = gesture.scrollLeft - (event.clientX - gesture.startX)
    event.currentTarget.scrollTop = gesture.scrollTop - (event.clientY - gesture.startY)
  }

  function handlePanEnd(event: ReactPointerEvent<HTMLDivElement>): void {
    const gesture = panGestureRef.current
    if (!gesture || gesture.pointerId !== event.pointerId) return
    event.preventDefault()
    event.stopPropagation()
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId)
    }
    panGestureRef.current = null
    setPanning(false)
  }

  return (
    <div className="buff-editor-zoom" aria-label={label}>
      <div
        className="buff-editor-zoom__viewport"
        data-panning={panning}
        data-zoom-viewport=""
        ref={viewportRef}
        onDoubleClickCapture={(event) => {
          if (!event.ctrlKey) return
          event.preventDefault()
          event.stopPropagation()
        }}
        onPointerCancelCapture={handlePanEnd}
        onPointerDownCapture={handlePanStart}
        onPointerMoveCapture={handlePanMove}
        onPointerUpCapture={handlePanEnd}
      >
        <div
          className="buff-editor-zoom__stage"
          style={stageWidth > 0 ? { width: stageWidth, height: stageHeight } : undefined}
        >
          <div
            className="buff-editor-zoom__content"
            data-zoom-content=""
            ref={contentRef}
            style={
              {
                '--editor-zoom': zoom,
                '--editor-zoom-inverse': 1 / displayScale,
                left: contentLeft,
                top: contentTop,
                transform: `scale(${displayScale})`
              } as CSSProperties
            }
          >
            {children}
          </div>
        </div>
      </div>
      <div className="buff-editor-zoom__toolbar">
        <span>滚轮缩放 · Ctrl+左键拖动 · {Math.round(zoom * 100)}%</span>
        <Button
          disabled={zoom === minimumZoom}
          size="compact"
          type="button"
          variant="outline"
          onClick={resetZoom}
        >
          重置为 100%
        </Button>
      </div>
    </div>
  )
}

export function calculateWheelZoom(current: number, deltaY: number): number {
  if (deltaY === 0) return current
  return round(clamp(current * Math.exp(-deltaY * 0.0015), minimumZoom, maximumZoom))
}

export function calculateFitScale(content: Dimensions, viewport: Dimensions): number {
  if (content.width <= 0 || content.height <= 0 || viewport.width <= 0 || viewport.height <= 0) {
    return 1
  }
  return Math.min(viewport.width / content.width, viewport.height / content.height)
}

export function getViewportDimensions(
  viewport: HTMLElement,
  entry?: ResizeObserverEntry
): Dimensions {
  const borderBox = entry?.borderBoxSize
  const box = Array.isArray(borderBox) ? borderBox[0] : borderBox
  const bounds = viewport.getBoundingClientRect()
  const styles = getComputedStyle(viewport)
  const horizontalBorder =
    parsePixelValue(styles.borderLeftWidth) + parsePixelValue(styles.borderRightWidth)
  const verticalBorder =
    parsePixelValue(styles.borderTopWidth) + parsePixelValue(styles.borderBottomWidth)

  return {
    width: Math.max(0, (box?.inlineSize ?? bounds.width) - horizontalBorder),
    height: Math.max(0, (box?.blockSize ?? bounds.height) - verticalBorder)
  }
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value))
}

function round(value: number): number {
  return Math.round(value * 10_000) / 10_000
}

function parsePixelValue(value: string): number {
  const parsed = Number.parseFloat(value)
  return Number.isFinite(parsed) ? parsed : 0
}
