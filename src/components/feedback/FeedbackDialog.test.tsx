import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useRef, useState } from 'react'
import { describe, expect, it, vi } from 'vitest'

import type { FeatureAccessStatus } from '../../lib/macro-api'
import { FeedbackDialog } from './FeedbackDialog'

type FeedbackHarnessProps = {
  restrictedWorkspacesUnlocked?: boolean
  onSubmit?: (content: string) => Promise<FeatureAccessStatus>
}

function FeedbackHarness({
  restrictedWorkspacesUnlocked = false,
  onSubmit = async () => ({ restrictedWorkspacesUnlocked: false })
}: FeedbackHarnessProps) {
  const [open, setOpen] = useState(true)
  const triggerRef = useRef<HTMLButtonElement>(null)

  return (
    <>
      <output data-testid="dialog-open">{String(open)}</output>
      <button ref={triggerRef} type="button" onClick={() => setOpen(true)}>
        意见反馈
      </button>
      <FeedbackDialog
        open={open}
        restrictedWorkspacesUnlocked={restrictedWorkspacesUnlocked}
        returnFocusRef={triggerRef}
        onOpenChange={setOpen}
        onSubmit={onSubmit}
      />
    </>
  )
}

describe('FeedbackDialog', () => {
  it('focuses the labeled field and keeps submit disabled for empty content', async () => {
    render(<FeedbackHarness />)

    const field = screen.getByRole('textbox', { name: '反馈内容' })
    await waitFor(() => expect(field).toHaveFocus())
    expect(screen.getByRole('button', { name: '提交反馈' })).toBeDisabled()
  })

  it('submits ordinary feedback without unlocking and closes after confirmation', async () => {
    const user = userEvent.setup()
    const onSubmit = vi.fn(async () => ({ restrictedWorkspacesUnlocked: false }))
    render(<FeedbackHarness onSubmit={onSubmit} />)

    await user.type(screen.getByRole('textbox', { name: '反馈内容' }), '希望增加更多提示')
    await user.click(screen.getByRole('button', { name: '提交反馈' }))

    expect(onSubmit).toHaveBeenCalledWith('希望增加更多提示')
    expect(await screen.findByRole('status')).toHaveTextContent('感谢反馈')

    await waitFor(() => expect(screen.getByTestId('dialog-open')).toHaveTextContent('false'), {
      timeout: 1500
    })
  })

  it('announces when the restricted workspaces are newly unlocked', async () => {
    const user = userEvent.setup()
    const onSubmit = vi.fn(async () => ({ restrictedWorkspacesUnlocked: true }))
    render(<FeedbackHarness onSubmit={onSubmit} />)

    await user.type(screen.getByRole('textbox', { name: '反馈内容' }), '大米米牛逼')
    await user.click(screen.getByRole('button', { name: '提交反馈' }))

    expect(await screen.findByRole('status')).toHaveTextContent('已开放宏流程和游戏录制')
  })

  it('announces when the restricted workspaces are hidden again', async () => {
    const user = userEvent.setup()
    const onSubmit = vi.fn(async () => ({ restrictedWorkspacesUnlocked: false }))
    render(<FeedbackHarness restrictedWorkspacesUnlocked onSubmit={onSubmit} />)

    await user.type(screen.getByRole('textbox', { name: '反馈内容' }), '大吉吉牛逼')
    await user.click(screen.getByRole('button', { name: '提交反馈' }))

    expect(await screen.findByRole('status')).toHaveTextContent('已隐藏宏流程和游戏录制')
  })

  it('keeps the dialog open and shows a useful error when submission fails', async () => {
    const user = userEvent.setup()
    const onSubmit = vi.fn(async () => {
      throw new Error('配置不可写')
    })
    render(<FeedbackHarness onSubmit={onSubmit} />)

    await user.type(screen.getByRole('textbox', { name: '反馈内容' }), '测试')
    await user.click(screen.getByRole('button', { name: '提交反馈' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('提交失败：配置不可写')
    expect(screen.getByTestId('dialog-open')).toHaveTextContent('true')
    expect(screen.getByRole('textbox', { name: '反馈内容' })).toBeEnabled()
  })

  it('closes with Escape and returns focus to the trigger', async () => {
    const user = userEvent.setup()
    render(<FeedbackHarness />)

    await user.keyboard('{Escape}')

    expect(screen.getByTestId('dialog-open')).toHaveTextContent('false')
    await waitFor(() => expect(screen.getByRole('button', { name: '意见反馈' })).toHaveFocus())
  })
})
