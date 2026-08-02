import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { RegionSelector } from './RegionSelector'

const initial = { x: 0.2, y: 0.2, width: 0.3, height: 0.3 }

function renderSelector(onChange = vi.fn(), onRequestExpand = vi.fn()) {
  const result = render(
    <RegionSelector
      imageUrl="preview.png"
      label="测试区域"
      value={initial}
      onChange={onChange}
      onRequestExpand={onRequestExpand}
    />
  )
  const canvas = screen.getByRole('application', { name: '测试区域' })
  vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    top: 0,
    left: 0,
    right: 1000,
    bottom: 500,
    width: 1000,
    height: 500,
    toJSON: () => ({})
  })
  return { ...result, canvas, onChange, onRequestExpand }
}

describe('RegionSelector', () => {
  it('does not replace the selection for a click without dragging', () => {
    const { canvas, onChange } = renderSelector()

    fireEvent.pointerDown(canvas, { button: 0, clientX: 50, clientY: 50, detail: 1 })
    fireEvent.pointerUp(canvas, { button: 0, clientX: 50, clientY: 50, detail: 1 })

    expect(onChange).not.toHaveBeenCalled()
  })

  it('creates a normalized rectangle by dragging the empty image area', () => {
    const { canvas, onChange } = renderSelector()

    fireEvent.pointerDown(canvas, { button: 0, clientX: 100, clientY: 50, detail: 1 })
    fireEvent.pointerMove(canvas, { clientX: 400, clientY: 250 })
    fireEvent.pointerUp(canvas, { clientX: 400, clientY: 250, detail: 1 })

    expect(onChange).toHaveBeenCalledWith({ x: 0.1, y: 0.1, width: 0.3, height: 0.4 })
  })

  it('keeps the minimum rectangle inside the image at the bottom-right edge', () => {
    const { canvas, onChange } = renderSelector()

    fireEvent.pointerDown(canvas, { button: 0, clientX: 995, clientY: 495, detail: 1 })
    fireEvent.pointerMove(canvas, { clientX: 1000, clientY: 500 })
    fireEvent.pointerUp(canvas, { clientX: 1000, clientY: 500, detail: 1 })

    expect(onChange).toHaveBeenCalledWith({ x: 0.99, y: 0.99, width: 0.01, height: 0.01 })
  })

  it('moves the existing rectangle and clamps it inside the image', () => {
    const { container, canvas, onChange } = renderSelector()
    const selection = container.querySelector('[data-region-selection]') as HTMLElement

    fireEvent.pointerDown(selection, { button: 0, clientX: 300, clientY: 150, detail: 1 })
    fireEvent.pointerMove(canvas, { clientX: 950, clientY: 490 })
    fireEvent.pointerUp(canvas, { clientX: 950, clientY: 490, detail: 1 })

    expect(onChange).toHaveBeenCalledWith({ x: 0.7, y: 0.7, width: 0.3, height: 0.3 })
  })

  it('resizes from the south-east handle', () => {
    const { container, canvas, onChange } = renderSelector()
    const handle = container.querySelector('[data-resize-handle="se"]') as HTMLElement

    fireEvent.pointerDown(handle, { button: 0, clientX: 500, clientY: 250, detail: 1 })
    fireEvent.pointerMove(canvas, { clientX: 800, clientY: 400 })
    fireEvent.pointerUp(canvas, { clientX: 800, clientY: 400, detail: 1 })

    expect(onChange).toHaveBeenCalledWith({ x: 0.2, y: 0.2, width: 0.6, height: 0.6 })
  })

  it('opens expanded editing on double click without changing the rectangle', () => {
    const { canvas, onChange, onRequestExpand } = renderSelector()

    fireEvent.doubleClick(canvas, { clientX: 250, clientY: 150 })

    expect(onRequestExpand).toHaveBeenCalledOnce()
    expect(onChange).not.toHaveBeenCalled()
  })

  it('keeps normalized selection coordinates correct at a scaled display size', () => {
    const onChange = vi.fn()
    render(
      <RegionSelector
        expanded
        imageUrl="preview.png"
        label="缩放区域"
        value={initial}
        onChange={onChange}
      />
    )
    const canvas = screen.getByRole('application', { name: '缩放区域' })
    vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({
      x: 0,
      y: 0,
      top: 0,
      left: 0,
      right: 2000,
      bottom: 1000,
      width: 2000,
      height: 1000,
      toJSON: () => ({})
    })

    fireEvent.pointerDown(canvas, { button: 0, clientX: 200, clientY: 100, detail: 1 })
    fireEvent.pointerMove(canvas, { clientX: 800, clientY: 400 })
    fireEvent.pointerUp(canvas, { clientX: 800, clientY: 400, detail: 1 })

    expect(onChange).toHaveBeenCalledWith({ x: 0.1, y: 0.1, width: 0.3, height: 0.3 })
  })
})
