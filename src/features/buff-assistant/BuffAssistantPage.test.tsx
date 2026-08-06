import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { useBuffAssistantController } from '@/hooks/useBuffAssistantController'
import { createMacroApi, installMacroApi } from '@/test/test-utils'

import { BuffAssistantPage } from './BuffAssistantPage'

function BuffAssistantHarness() {
  const controller = useBuffAssistantController()
  return <BuffAssistantPage controller={controller} />
}

describe('BuffAssistantPage', () => {
  it('saves the overlay capture exclusion only after saving settings', async () => {
    const user = userEvent.setup()
    const api = createMacroApi()
    installMacroApi(api)
    render(<BuffAssistantHarness />)

    const captureToggle = await screen.findByRole('checkbox', { name: '排除录屏捕获' })
    expect(captureToggle).not.toBeChecked()

    await user.click(captureToggle)
    expect(api.updateBuffAssistantSettings).not.toHaveBeenCalled()

    await user.click(screen.getByRole('button', { name: '保存设置' }))

    await waitFor(() => {
      expect(api.updateBuffAssistantSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          overlay: expect.objectContaining({ excludeFromCapture: true })
        })
      )
    })
  })

  it('requests access and saves the system capture border preference', async () => {
    const user = userEvent.setup()
    const api = createMacroApi()
    installMacroApi(api)
    render(<BuffAssistantHarness />)

    const borderToggle = await screen.findByRole('checkbox', {
      name: '显示系统捕获黄色边框'
    })
    expect(borderToggle).toBeChecked()

    await user.click(borderToggle)
    await user.selectOptions(screen.getByRole('combobox', { name: '浮窗配色' }), 'blackWhite')
    await user.click(screen.getByRole('button', { name: '保存设置' }))

    await waitFor(() => {
      expect(api.updateBuffAssistantSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          capture: expect.objectContaining({ showSystemBorder: false }),
          overlay: expect.objectContaining({ colorScheme: 'blackWhite' })
        })
      )
    })
    expect(api.requestBuffBorderlessCaptureAccess).toHaveBeenCalledOnce()
  })

  it('keeps the border enabled when Windows denies borderless access', async () => {
    const user = userEvent.setup()
    const api = createMacroApi()
    api.requestBuffBorderlessCaptureAccess = vi.fn(async () => 'deniedByUser')
    installMacroApi(api)
    render(<BuffAssistantHarness />)

    const borderToggle = await screen.findByRole('checkbox', {
      name: '显示系统捕获黄色边框'
    })
    await user.click(borderToggle)

    expect(borderToggle).toBeChecked()
    expect(
      await screen.findByText('未获得隐藏系统捕获边框的用户授权，已继续显示黄色边框')
    ).toBeVisible()
  })

  it('disables the border switch when the system API is unavailable', async () => {
    const api = createMacroApi()
    api.getBuffAssistantState = vi.fn(async () => ({
      ...(await createMacroApi().getBuffAssistantState()),
      captureBorderSupported: false
    }))
    installMacroApi(api)
    render(<BuffAssistantHarness />)

    expect(await screen.findByRole('checkbox', { name: '显示系统捕获黄色边框' })).toBeDisabled()
    expect(screen.getByText('当前 Windows 版本不支持隐藏系统捕获黄色边框。')).toBeVisible()
  })

  it('configures and previews each sound cue independently', async () => {
    const user = userEvent.setup()
    const api = createMacroApi()
    api.importBuffAssistantSound.mockResolvedValue({
      assetId: 'prewarn-one-123',
      fileName: '我的一.wav'
    })
    installMacroApi(api)
    render(<BuffAssistantHarness />)

    expect(await screen.findByRole('checkbox', { name: '真实触发确认音' })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: '倒计时 3 秒提示音' })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: '倒计时 2 秒提示音' })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: '倒计时 1 秒提示音' })).toBeChecked()

    const threeSource = screen.getByRole('combobox', { name: '倒计时 3 秒提示音来源' })
    await waitFor(() => expect(threeSource).toHaveTextContent('模板一'))
    await user.selectOptions(threeSource, 'template:template-1')
    await user.click(screen.getByRole('button', { name: '试听倒计时 3 秒提示音' }))

    expect(api.playBuffAssistantSound).toHaveBeenCalledWith(
      'prewarnThree',
      { type: 'template', templateId: 'template-1' },
      0.45
    )

    await user.click(screen.getByRole('button', { name: '上传倒计时 1 秒提示音 WAV' }))
    expect(api.importBuffAssistantSound).toHaveBeenCalledWith('prewarnOne')
    expect(screen.getByRole('combobox', { name: '倒计时 1 秒提示音来源' })).toHaveValue(
      'custom:prewarn-one-123'
    )

    await user.click(screen.getByRole('button', { name: '保存设置' }))
    await waitFor(() => {
      expect(api.updateBuffAssistantSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          sound: expect.objectContaining({
            prewarnThreeSource: { type: 'template', templateId: 'template-1' },
            prewarnOneSource: {
              type: 'custom',
              assetId: 'prewarn-one-123',
              fileName: '我的一.wav'
            }
          })
        })
      )
    })
  })

  it('offers the fixed TTS Online helper', async () => {
    const user = userEvent.setup()
    const api = createMacroApi()
    installMacroApi(api)
    render(<BuffAssistantHarness />)

    expect(await screen.findByText(/可前往 TTS Online 将文本转换为语音/)).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '前往 TTS Online' }))
    expect(api.openTtsOnline).toHaveBeenCalledTimes(1)
  })
})
