import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'

import { createBuffSentinelApi, installBuffSentinelApi } from '../../test/test-utils'
import { WindowTitleBar } from './WindowTitleBar'

describe('WindowTitleBar', () => {
  it('places the open-source notice after the update action', async () => {
    const user = userEvent.setup()
    const api = createBuffSentinelApi()
    installBuffSentinelApi(api)
    render(<WindowTitleBar onCheckForUpdate={() => undefined} />)

    const updateButton = screen.getByRole('button', { name: '检查更新' })
    const noticeButton = screen.getByRole('button', { name: '开源免费 · 声明' })
    expect(
      updateButton.compareDocumentPosition(noticeButton) & Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy()

    await user.click(noticeButton)
    const dialog = await screen.findByRole('dialog', { name: '开源声明与支持开发' })
    expect(dialog).toHaveTextContent(
      '本软件开源、完全免费。凡是对外收费售卖本软件均为第三方倒卖，请谨防被骗。'
    )
    expect(dialog).toHaveTextContent('打赏纯属自愿，不强制，不提供特权，感谢支持开发者！')
    expect(dialog).toHaveTextContent('作者：401163814@qq.com')
    expect(within(dialog).getByRole('img', { name: '开发者微信支付收款码' })).toBeVisible()

    const author = within(dialog).getByText('作者：401163814@qq.com')
    expect(fireEvent.mouseDown(author, { button: 0 })).toBe(true)
    expect(api.window.startDragging).not.toHaveBeenCalled()

    await user.click(within(dialog).getByRole('button', { name: '关闭' }))
    await waitFor(() => expect(dialog).not.toBeInTheDocument())
    expect(noticeButton).not.toHaveFocus()
    expect(api.window.startDragging).not.toHaveBeenCalled()
  })
})
