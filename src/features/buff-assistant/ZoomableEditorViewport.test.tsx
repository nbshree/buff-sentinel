import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { calculateWheelZoom, ZoomableEditorViewport } from './ZoomableEditorViewport'

describe('calculateWheelZoom', () => {
  it('zooms smoothly and clamps the result between 100% and 800%', () => {
    expect(calculateWheelZoom(1, -100)).toBeGreaterThan(1)
    expect(calculateWheelZoom(1, 100)).toBe(1)
    expect(calculateWheelZoom(7.9, -10_000)).toBe(8)
    expect(calculateWheelZoom(1.1, 10_000)).toBe(1)
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
    expect(screen.getByText('滚轮缩放 · 200%')).toBeInTheDocument()
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

    expect(screen.getByText('滚轮缩放 · 100%')).toBeInTheDocument()
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

    expect(screen.getByText('滚轮缩放 · 100%')).toBeInTheDocument()
    expect(viewport.scrollLeft).toBe(0)
    expect(viewport.scrollTop).toBe(0)
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
