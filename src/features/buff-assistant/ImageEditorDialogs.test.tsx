import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { appendMaskStroke, createMaskHistory, type MaskStroke } from './MaskEditor'
import { MaskEditorDialog } from './MaskEditorDialog'
import { RegionEditorDialog } from './RegionEditorDialog'

const crop = { x: 0.2, y: 0.2, width: 0.3, height: 0.3 }
const stroke: MaskStroke = { points: [{ x: 0.4, y: 0.5 }], radius: 0.08 }

describe('RegionEditorDialog', () => {
  it('keeps edits as a draft until apply is clicked', () => {
    const onApply = vi.fn()
    const onOpenChange = vi.fn()
    render(
      <RegionEditorDialog
        description="说明"
        imageUrl="preview.png"
        label="Buff 搜索区域"
        open
        title="精调区域"
        value={crop}
        onApply={onApply}
        onOpenChange={onOpenChange}
      />
    )
    const canvas = screen.getByRole('application', { name: 'Buff 搜索区域' })
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

    fireEvent.pointerDown(canvas, { button: 0, clientX: 100, clientY: 50, detail: 1 })
    fireEvent.pointerMove(canvas, { clientX: 500, clientY: 300 })
    fireEvent.pointerUp(canvas, { clientX: 500, clientY: 300, detail: 1 })
    expect(onApply).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: '应用区域' }))
    expect(onApply).toHaveBeenCalledWith({ x: 0.1, y: 0.1, width: 0.4, height: 0.5 })
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })

  it('discards the draft when cancel is clicked', () => {
    const onApply = vi.fn()
    const onOpenChange = vi.fn()
    render(
      <RegionEditorDialog
        description="说明"
        imageUrl="preview.png"
        label="Buff 搜索区域"
        open
        title="精调区域"
        value={crop}
        onApply={onApply}
        onOpenChange={onOpenChange}
      />
    )

    fireEvent.click(screen.getByRole('button', { name: '取消' }))

    expect(onApply).not.toHaveBeenCalled()
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })
})

describe('MaskEditorDialog', () => {
  it('applies its isolated mask draft only after confirmation', async () => {
    const onApply = vi.fn()
    const onOpenChange = vi.fn()
    render(
      <MaskEditorDialog
        crop={{ x: 0, y: 0, width: 1, height: 1 }}
        imageUrl="template.png"
        open
        value={appendMaskStroke(createMaskHistory(), stroke)}
        onApply={onApply}
        onOpenChange={onOpenChange}
      />
    )

    const clear = screen.getByRole('button', { name: '清除遮罩' })
    await waitFor(() => expect(clear).toBeEnabled())
    fireEvent.click(clear)
    expect(onApply).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: '应用遮罩' }))
    expect(onApply).toHaveBeenCalledWith(expect.objectContaining({ present: [] }))
    expect(onOpenChange).toHaveBeenCalledWith(false)
  })
})
