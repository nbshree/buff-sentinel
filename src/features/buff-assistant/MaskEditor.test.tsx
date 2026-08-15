import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import {
  appendMaskStroke,
  brushRadiusFromDiameter,
  clearMaskHistory,
  createMaskHistory,
  imagePixelFromClientPoint,
  maskPixelsMatchingColor,
  MaskEditor,
  replaceMaskHistory,
  segmentForegroundByBorderColor,
  undoMaskHistory,
  type MaskStroke
} from './MaskEditor'

const firstStroke: MaskStroke = { points: [{ x: 0.2, y: 0.3 }], radius: 0.08 }
const secondStroke: MaskStroke = {
  points: [
    { x: 0.4, y: 0.5 },
    { x: 0.6, y: 0.7 }
  ],
  radius: 0.08
}
const crop = { x: 0, y: 0, width: 1, height: 1 }

describe('mask history', () => {
  it('undoes one complete stroke at a time', () => {
    const withFirst = appendMaskStroke(createMaskHistory(), firstStroke)
    const withSecond = appendMaskStroke(withFirst, secondStroke)

    expect(undoMaskHistory(withSecond).present).toEqual([firstStroke])
    expect(undoMaskHistory(undoMaskHistory(withSecond)).present).toEqual([])
  })

  it('allows clearing the mask to be undone', () => {
    const painted = appendMaskStroke(createMaskHistory(), firstStroke)
    const cleared = clearMaskHistory(painted)

    expect(cleared.present).toEqual([])
    expect(undoMaskHistory(cleared).present).toEqual([firstStroke])
  })

  it('allows a persisted mask to be cleared and restored', () => {
    const persisted = createMaskHistory('data:image/png;base64,bWFzaw==')
    const cleared = clearMaskHistory(persisted)

    expect(cleared.baseMaskDataUrl).toBeNull()
    expect(undoMaskHistory(cleared).baseMaskDataUrl).toBe(persisted.baseMaskDataUrl)
  })

  it('allows an AI mask to replace the current mask and be undone', () => {
    const painted = appendMaskStroke(createMaskHistory('old-mask'), firstStroke)
    const generated = replaceMaskHistory(painted, 'ai-mask')

    expect(generated.baseMaskDataUrl).toBe('ai-mask')
    expect(generated.present).toEqual([])
    expect(undoMaskHistory(generated).baseMaskDataUrl).toBe('old-mask')
    expect(undoMaskHistory(generated).present).toEqual([firstStroke])
  })
})

describe('mask brush sizing', () => {
  it('keeps brush diameter tied to original image pixels', () => {
    expect(brushRadiusFromDiameter(1, 1144, 342)).toBeCloseTo(1 / 684)
    expect(brushRadiusFromDiameter(3, 1144, 342)).toBeCloseTo(3 / 684)
  })
})

describe('mask canvas coordinates', () => {
  it('maps a client point to the same source pixel after zooming and panning', () => {
    expect(
      imagePixelFromClientPoint(
        0,
        0,
        { left: -400, top: -200, width: 2000, height: 1000 },
        1000,
        500
      )
    ).toEqual({ x: 200, y: 100 })
  })
})

describe('color boundary segmentation', () => {
  it('keeps the largest contrasting subject and ignores the border background', () => {
    const width = 8
    const height = 8
    const pixels = new Uint8ClampedArray(width * height * 4)
    for (let pixel = 0; pixel < width * height; pixel += 1) {
      const offset = pixel * 4
      pixels[offset] = 18
      pixels[offset + 1] = 43
      pixels[offset + 2] = 64
      pixels[offset + 3] = 255
    }
    for (let y = 2; y <= 5; y += 1) {
      for (let x = 2; x <= 5; x += 1) {
        const offset = (y * width + x) * 4
        pixels[offset] = 255
        pixels[offset + 1] = 174
        pixels[offset + 2] = 57
      }
    }

    const result = segmentForegroundByBorderColor(pixels, width, height)

    expect(result.maskPixels[0]).toBe(0)
    expect(result.maskPixels[3 * width + 3]).toBe(255)
    expect(result.ignoredPercent).toBe(75)
  })
})

describe('picked color masking', () => {
  const sourcePixels = new Uint8ClampedArray([
    255, 255, 255, 255, 240, 245, 250, 255, 40, 40, 40, 255, 255, 255, 255, 0
  ])

  it('matches exact colors globally and preserves the existing mask', () => {
    const currentMask = new Uint8ClampedArray([255, 0, 255, 255])

    const result = maskPixelsMatchingColor(sourcePixels, currentMask, 4, 1, 0, 0, 0)

    expect(result.color).toEqual([255, 255, 255])
    expect(result.maskPixels).toEqual(new Uint8ClampedArray([0, 0, 255, 255]))
    expect(result.matchedCount).toBe(1)
  })

  it('includes disconnected similar colors but skips transparent pixels', () => {
    const result = maskPixelsMatchingColor(
      sourcePixels,
      new Uint8ClampedArray([255, 255, 255, 255]),
      4,
      1,
      0,
      0,
      32
    )

    expect(result.maskPixels).toEqual(new Uint8ClampedArray([0, 0, 255, 255]))
    expect(result.matchedCount).toBe(2)
  })

  it('rejects a transparent sample point', () => {
    expect(() =>
      maskPixelsMatchingColor(
        sourcePixels,
        new Uint8ClampedArray([255, 255, 255, 255]),
        4,
        1,
        3,
        0,
        32
      )
    ).toThrow('该像素透明')
  })
})

describe('MaskEditor', () => {
  it('shows color picking controls only in expanded editing', () => {
    const { rerender } = render(
      <MaskEditor
        crop={crop}
        imageUrl="template.png"
        value={createMaskHistory()}
        onChange={vi.fn()}
      />
    )

    expect(screen.queryByRole('button', { name: '按颜色涂抹' })).not.toBeInTheDocument()
    rerender(
      <MaskEditor
        expanded
        crop={crop}
        imageUrl="template.png"
        value={createMaskHistory()}
        onChange={vi.fn()}
      />
    )
    expect(screen.getByRole('button', { name: '按颜色涂抹' })).toBeInTheDocument()
    expect(screen.getByRole('slider', { name: '相似颜色容差' })).toHaveValue('32')
  })

  it('offers a one-pixel brush for precise mask editing', () => {
    render(
      <MaskEditor
        crop={crop}
        imageUrl="template.png"
        value={createMaskHistory()}
        onChange={vi.fn()}
      />
    )

    const brush = screen.getByRole('slider', { name: '遮罩笔刷大小' })
    expect(brush).toHaveAttribute('min', '1')
    expect(brush).toHaveValue('3')
    fireEvent.change(brush, { target: { value: '1' } })
    expect(brush).toHaveValue('1')
  })

  it('uses the undo button and scoped Ctrl+Z to restore the previous stroke state', () => {
    const empty = createMaskHistory()
    const oneStroke = appendMaskStroke(empty, firstStroke)
    const twoStrokes = appendMaskStroke(oneStroke, secondStroke)
    const onChange = vi.fn()
    const { container, rerender } = render(
      <MaskEditor crop={crop} imageUrl="template.png" value={twoStrokes} onChange={onChange} />
    )

    fireEvent.click(screen.getByRole('button', { name: '撤销上一笔遮罩' }))
    expect(onChange).toHaveBeenLastCalledWith(oneStroke)

    rerender(
      <MaskEditor crop={crop} imageUrl="template.png" value={oneStroke} onChange={onChange} />
    )
    fireEvent.keyDown(container.querySelector('.buff-mask-editor') as HTMLElement, {
      key: 'z',
      ctrlKey: true
    })
    expect(onChange).toHaveBeenLastCalledWith(empty)
  })

  it('does not add a stroke when double click requests expanded editing', () => {
    const onChange = vi.fn()
    const onRequestExpand = vi.fn()
    render(
      <MaskEditor
        crop={crop}
        imageUrl="template.png"
        value={createMaskHistory()}
        onChange={onChange}
        onRequestExpand={onRequestExpand}
      />
    )

    fireEvent.doubleClick(screen.getByLabelText('模板忽略区域画笔'))

    expect(onRequestExpand).toHaveBeenCalledOnce()
    expect(onChange).not.toHaveBeenCalled()
  })
})
