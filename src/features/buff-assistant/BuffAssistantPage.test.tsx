import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { useBuffAssistantController } from '@/hooks/useBuffAssistantController'
import { createBuffSentinelApi, installBuffSentinelApi } from '@/test/test-utils'

import { BuffAssistantPage } from './BuffAssistantPage'

function BuffAssistantHarness() {
  const controller = useBuffAssistantController()
  return <BuffAssistantPage controller={controller} />
}

async function openSettings(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('button', { name: '识别与提醒设置' }))
  return screen.findByRole('dialog')
}

describe('BuffAssistantPage', () => {
  it('shows setting guidance in accessible tooltips', async () => {
    const user = userEvent.setup()
    const api = createBuffSentinelApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)
    await openSettings(user)

    expect(screen.getByRole('spinbutton', { name: /触发宽限期/ })).toHaveValue(1500)
    const graceTooltip = await screen.findByRole('button', { name: '查看触发宽限期说明' })
    await user.hover(graceTooltip)
    expect(
      await screen.findByText('单位：毫秒，建议值 1500', {
        selector: '[data-slot="tooltip-content"]'
      })
    ).toBeVisible()
  })

  it('shows the capture exclusion guidance in a tooltip', async () => {
    const user = userEvent.setup()
    const api = createBuffSentinelApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)
    await openSettings(user)

    const captureTooltip = screen.getByRole('button', { name: '查看排除录屏捕获说明' })
    await user.hover(captureTooltip)
    expect(
      await screen.findByText(/开启后，OBS 等使用系统捕获接口的工具通常不会录入 Buff 悬浮窗/, {
        selector: '[data-slot="tooltip-content"]'
      })
    ).toBeVisible()
  })

  it('saves the overlay capture exclusion only after confirmation', async () => {
    const user = userEvent.setup()
    const api = createBuffSentinelApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)
    await openSettings(user)

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
    const api = createBuffSentinelApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)
    await openSettings(user)

    const borderToggle = await screen.findByRole('checkbox', {
      name: '隐藏系统捕获黄色边框'
    })
    expect(borderToggle).not.toBeChecked()

    await user.click(borderToggle)
    await user.selectOptions(screen.getByRole('combobox', { name: '浮窗配色' }), 'blackWhite')
    expect(api.updateBuffAssistantSettings).not.toHaveBeenCalled()
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
    const api = createBuffSentinelApi()
    api.requestBuffBorderlessCaptureAccess = vi.fn(async () => 'deniedByUser')
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)
    await openSettings(user)

    const borderToggle = await screen.findByRole('checkbox', {
      name: '隐藏系统捕获黄色边框'
    })
    await user.click(borderToggle)

    expect(borderToggle).not.toBeChecked()
    expect(
      await screen.findByText('未获得隐藏系统捕获边框的用户授权，已继续显示黄色边框')
    ).toBeVisible()
  })

  it('shows the system capture border again without requesting access', async () => {
    const user = userEvent.setup()
    const api = createBuffSentinelApi()
    const initialState = await api.getBuffAssistantState()
    api.getBuffAssistantState = vi.fn(async () => ({
      ...initialState,
      config: {
        ...initialState.config,
        settings: {
          ...initialState.config.settings,
          capture: { showSystemBorder: false }
        }
      }
    }))
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)
    await openSettings(user)

    const borderToggle = await screen.findByRole('checkbox', {
      name: '隐藏系统捕获黄色边框'
    })
    await waitFor(() => expect(borderToggle).toBeChecked())
    await user.click(borderToggle)
    expect(api.updateBuffAssistantSettings).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: '保存设置' }))

    await waitFor(() => {
      expect(api.updateBuffAssistantSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          capture: expect.objectContaining({ showSystemBorder: true })
        })
      )
    })
    expect(api.requestBuffBorderlessCaptureAccess).not.toHaveBeenCalled()
  })

  it('disables the border switch when the system API is unavailable', async () => {
    const user = userEvent.setup()
    const api = createBuffSentinelApi()
    api.getBuffAssistantState = vi.fn(async () => ({
      ...(await createBuffSentinelApi().getBuffAssistantState()),
      captureBorderSupported: false
    }))
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)
    await openSettings(user)

    expect(await screen.findByRole('checkbox', { name: '隐藏系统捕获黄色边框' })).toBeDisabled()
    const borderTooltip = screen.getByRole('button', {
      name: '查看隐藏系统捕获黄色边框说明'
    })
    await user.hover(borderTooltip)
    expect(
      await screen.findByText('当前 Windows 版本不支持隐藏系统捕获黄色边框。', {
        selector: '[data-slot="tooltip-content"]'
      })
    ).toBeVisible()
  })

  it('configures and previews each sound cue independently', async () => {
    const user = userEvent.setup()
    const api = createBuffSentinelApi()
    api.importBuffAssistantSound.mockResolvedValue({
      assetId: 'prewarn-one-123',
      fileName: '我的一.wav'
    })
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)
    await openSettings(user)

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
    expect(api.updateBuffAssistantSettings).not.toHaveBeenCalled()
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

  it('keeps monitoring controls outside the settings dialog', async () => {
    const user = userEvent.setup()
    const api = createBuffSentinelApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    expect(await screen.findByRole('button', { name: '开始监控' })).toBeVisible()
    expect(screen.getByRole('button', { name: '调整悬浮位置' })).toBeVisible()
    const dialog = await openSettings(user)
    expect(within(dialog).getByRole('button', { name: '保存设置' })).toBeVisible()
    expect(within(dialog).queryByRole('button', { name: '开始监控' })).not.toBeInTheDocument()
    expect(within(dialog).queryByRole('button', { name: '调整悬浮位置' })).not.toBeInTheDocument()
    expect(screen.queryByText('自动监听真实触发，脱战后自动丢弃旧时间轴')).not.toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: '日常监控' })).not.toBeInTheDocument()
  })

  it('discards draft settings when cancelled', async () => {
    const user = userEvent.setup()
    const api = createBuffSentinelApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)
    await openSettings(user)

    const captureToggle = screen.getByRole('checkbox', { name: '排除录屏捕获' })
    await user.click(captureToggle)
    await user.click(screen.getByRole('button', { name: '取消' }))
    expect(api.updateBuffAssistantSettings).not.toHaveBeenCalled()

    await openSettings(user)
    expect(screen.getByRole('checkbox', { name: '排除录屏捕获' })).not.toBeChecked()
  })

  it('offers the fixed TTS Online helper', async () => {
    const user = userEvent.setup()
    const api = createBuffSentinelApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)
    await openSettings(user)

    expect(await screen.findByText(/可前往 TTS Online 将文本转换为语音/)).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '前往 TTS Online' }))
    expect(api.openTtsOnline).toHaveBeenCalledTimes(1)
  })
})
