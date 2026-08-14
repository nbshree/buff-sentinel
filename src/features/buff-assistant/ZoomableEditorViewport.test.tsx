import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import {
  calculateFitScale,
  calculateWheelZoom,
  getViewportDimensions,
  ZoomableEditorViewport
} from './ZoomableEditorViewport'

describe('calculateFitScale', () => {
  it('fits the whole image and leaves space on the unconstrained axis', () => {
    const landscapeScale = calculateFitScale(
      { width: 1920, height: 1080 },
      { width: 1000, height: 500 }
    )
    expect(1920 * landscapeScale).toBeLessThan(1000)
    expect(1080 * landscapeScale).toBeCloseTo(500)

    const wideScale = calculateFitScale({ width: 2000, height: 500 }, { width: 1000, height: 500 })
    expect(2000 * wideScale).toBeCloseTo(1000)
    expect(500 * wideScale).toBeLessThan(500)
  })

  it('can enlarge a small image while still fitting it entirely', () => {
    expect(calculateFitScale({ width: 100, height: 50 }, { width: 1000, height: 500 })).toBe(10)
  })
})

describe('calculateWheelZoom', () => {
  it('zooms smoothly and clamps the result between 100% and 800%', () => {
    expect(calculateWheelZoom(1, -100)).toBeGreaterThan(1)
    expect(calculateWheelZoom(1, 100)).toBe(1)
    expect(calculateWheelZoom(7.9, -10_000)).toBe(8)
    expect(calculateWheelZoom(1.1, 10_000)).toBe(1)
  })
})

describe('getViewportDimensions', () => {
  it('uses the stable border box when scrollbars reduce the observed content box', () => {
    const viewport = document.createElement('div')
    viewport.style.border = '1px solid transparent'
    vi.spyOn(viewport, 'getBoundingClientRect').mockReturnValue(rect(0, 0, 1000, 600))
    const entry = {
      borderBoxSize: [{ inlineSize: 1000, blockSize: 600 }],
      contentRect: rect(0, 0, 983, 583)
    } as unknown as ResizeObserverEntry

    expect(getViewportDimensions(viewport, entry)).toEqual({ width: 998, height: 598 })
  })
})

describe('ZoomableEditorViewport', () => {
  it('keeps the pixel below the cursor anchored while wheel zooming', () => {
    const { container } = render(
      <ZoomableEditorViewport label="测试缩放视口" resetKey="image-a">
        <div>图片</div>
      </ZoomableEditorViewport>
    )
    const viewport = container.querySelector('[data-zoom-viewport]') as HTMLDivElement
    const content = container.querySelector('[data-zoom-content]') as HTMLDivElement
    viewport.scrollLeft = 10
    viewport.scrollTop = 20
    vi.spyOn(content, 'getBoundingClientRect')
      .mockReturnValueOnce(rect(0, 0, 100, 100))
      .mockReturnValue(rect(0, 0, 200, 200))

    const accepted = fireEvent.wheel(viewport, {
      clientX: 50,
      clientY: 50,
      deltaY: -462.1
    })

    expect(accepted).toBe(false)
    expect(screen.getByText(/200%$/)).toBeInTheDocument()
    expect(viewport.scrollLeft).toBe(60)
    expect(viewport.scrollTop).toBe(70)
  })

  it('resets zoom and scroll position to 100%', () => {
    const { container } = render(
      <ZoomableEditorViewport label="测试缩放视口" resetKey="image-a">
        <div>图片</div>
      </ZoomableEditorViewport>
    )
    const viewport = container.querySelector('[data-zoom-viewport]') as HTMLDivElement
    const content = container.querySelector('[data-zoom-content]') as HTMLDivElement
    vi.spyOn(content, 'getBoundingClientRect').mockReturnValue(rect(0, 0, 100, 100))
    fireEvent.wheel(viewport, { clientX: 50, clientY: 50, deltaY: -200 })
    viewport.scrollLeft = 120
    viewport.scrollTop = 80

    fireEvent.click(screen.getByRole('button', { name: '重置为 100%' }))

    expect(screen.getByText(/^滚轮缩放.*100%$/)).toBeInTheDocument()
    expect(viewport.scrollLeft).toBe(0)
    expect(viewport.scrollTop).toBe(0)
  })

  it('resets automatically when the edited image changes', () => {
    const { container, rerender } = render(
      <ZoomableEditorViewport label="测试缩放视口" resetKey="image-a">
        <div>图片 A</div>
      </ZoomableEditorViewport>
    )
    const viewport = container.querySelector('[data-zoom-viewport]') as HTMLDivElement
    const content = container.querySelector('[data-zoom-content]') as HTMLDivElement
    vi.spyOn(content, 'getBoundingClientRect').mockReturnValue(rect(0, 0, 100, 100))
    fireEvent.wheel(viewport, { clientX: 50, clientY: 50, deltaY: -200 })
    viewport.scrollLeft = 90
    viewport.scrollTop = 60

    rerender(
      <ZoomableEditorViewport label="测试缩放视口" resetKey="image-b">
        <div>图片 B</div>
      </ZoomableEditorViewport>
    )

    expect(screen.getByText(/^滚轮缩放.*100%$/)).toBeInTheDocument()
    expect(viewport.scrollLeft).toBe(0)
    expect(viewport.scrollTop).toBe(0)
  })

  it('pans the viewport with Ctrl and the left mouse button without reaching the editor', () => {
    const onPointerDown = vi.fn()
    const { container } = render(
      <ZoomableEditorViewport label="测试缩放视口" resetKey="image-a">
        <button type="button" onPointerDown={onPointerDown}>
          图片
        </button>
      </ZoomableEditorViewport>
    )
    const viewport = container.querySelector('[data-zoom-viewport]') as HTMLDivElement
    const target = screen.getByRole('button', { name: '图片' })
    viewport.scrollLeft = 200
    viewport.scrollTop = 150

    fireEvent.pointerDown(target, {
      button: 0,
      clientX: 100,
      clientY: 100,
      ctrlKey: true,
      pointerId: 7
    })
    expect(viewport).toHaveAttribute('data-panning', 'true')
    expect(onPointerDown).not.toHaveBeenCalled()

    fireEvent.pointerMove(viewport, { clientX: 130, clientY: 140, pointerId: 7 })
    expect(viewport.scrollLeft).toBe(170)
    expect(viewport.scrollTop).toBe(110)

    fireEvent.pointerUp(viewport, { clientX: 130, clientY: 140, pointerId: 7 })
    expect(viewport).toHaveAttribute('data-panning', 'false')
  })

  it('leaves ordinary left-button gestures to the editor', () => {
    const onPointerDown = vi.fn()
    render(
      <ZoomableEditorViewport label="测试缩放视口" resetKey="image-a">
        <button type="button" onPointerDown={onPointerDown}>
          图片
        </button>
      </ZoomableEditorViewport>
    )

    fireEvent.pointerDown(screen.getByRole('button', { name: '图片' }), {
      button: 0,
      clientX: 100,
      clientY: 100,
      pointerId: 8
    })

    expect(onPointerDown).toHaveBeenCalledOnce()
  })
})

function rect(left: number, top: number, width: number, height: number): DOMRect {
  return {
    x: left,
    y: top,
    top,
    left,
    right: left + width,
    bottom: top + height,
    width,
    height,
    toJSON: () => ({})
  }
}
