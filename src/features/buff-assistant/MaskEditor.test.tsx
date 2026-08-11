import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import {
  appendMaskStroke,
  clearMaskHistory,
  createMaskHistory,
  MaskEditor,
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
})

describe('MaskEditor', () => {
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
