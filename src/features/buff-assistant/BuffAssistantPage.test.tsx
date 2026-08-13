import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'

import { useBuffAssistantController } from '@/hooks/useBuffAssistantController'
import { createBuffSentinelApi, installBuffSentinelApi } from '@/test/test-utils'

import { BuffAssistantPage } from './BuffAssistantPage'

function BuffAssistantHarness() {
  const controller = useBuffAssistantController()
  return <BuffAssistantPage controller={controller} />
}

async function createListenerApi() {
  const baseApi = createBuffSentinelApi()
  const baseState = await baseApi.getBuffAssistantState()
  return createBuffSentinelApi({
    ...baseState,
    config: {
      ...baseState.config,
      target: {
        processName: 'game.exe',
        windowTitle: 'Game',
        className: 'GameWindow',
        referenceWidth: 1920,
        referenceHeight: 1080
      },
      searchRegion: { x: 0.5, y: 0, width: 0.4, height: 0.2 },
      listeners: [
        {
          id: 'jinzhoutian',
          name: '金周天',
          enabled: true,
          template: { id: 'template', width: 32, height: 32 },
          settings: {
            cycleMs: 20_000,
            deadlineGraceMs: 1500,
            threshold: 0.95,
            confirmFrames: 3,
            missingFrames: 5,
            sound: {
              triggerEnabled: true,
              prewarnThreeEnabled: true,
              prewarnTwoEnabled: true,
              prewarnOneEnabled: true,
              triggerSource: { type: 'sine' },
              prewarnThreeSource: { type: 'sine' },
              prewarnTwoSource: { type: 'sine' },
              prewarnOneSource: { type: 'sine' },
              volume: 0.45
            }
          }
        }
      ]
    },
    listeners: [
      {
        id: 'jinzhoutian',
        activity: 'stopped',
        expectedAtUnixMs: null,
        lastConfidence: 0,
        lastError: null
      }
    ]
  })
}

async function openGlobalSettings(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('button', { name: '识别与提醒设置' }))
  return screen.findByRole('dialog', { name: '识别与提醒设置' })
}

async function openListenerEditor(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('button', { name: '编辑' }))
  return screen.findByRole('dialog', { name: '编辑监听图标' })
}

describe('BuffAssistantPage', () => {
  it('keeps global capture settings in the top-level dialog', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openGlobalSettings(user)
    expect(within(dialog).getByRole('checkbox', { name: '排除录屏捕获' })).toBeVisible()
    expect(within(dialog).queryByRole('spinbutton', { name: /触发宽限期/ })).toBeNull()

    await user.click(within(dialog).getByRole('checkbox', { name: '排除录屏捕获' }))
    await user.click(within(dialog).getByRole('button', { name: '保存设置' }))
    await waitFor(() =>
      expect(api.updateBuffAssistantSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          overlay: expect.objectContaining({ excludeFromCapture: true })
        })
      )
    )
  })

  it('edits recognition, timing and sound settings per listener', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openListenerEditor(user)
    expect(within(dialog).getByRole('spinbutton', { name: /触发宽限期/ })).toHaveValue(1500)
    expect(within(dialog).getByRole('checkbox', { name: '真实触发确认音' })).toBeChecked()
    await user.clear(within(dialog).getByRole('spinbutton', { name: '周期（秒）' }))
    await user.type(within(dialog).getByRole('spinbutton', { name: '周期（秒）' }), '30')
    await user.click(within(dialog).getByRole('button', { name: '保存监听项' }))

    await waitFor(() =>
      expect(api.updateBuffListener).toHaveBeenCalledWith(
        'jinzhoutian',
        '金周天',
        true,
        expect.objectContaining({ cycleMs: 30_000 })
      )
    )
  })

  it('reloads the saved icon and mask when editing a listener', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openListenerEditor(user)

    await waitFor(() => expect(api.getBuffListenerTemplate).toHaveBeenCalledWith('jinzhoutian'))
    expect(await within(dialog).findByText('裁剪图标主体')).toBeVisible()
    expect(within(dialog).getByRole('application', { name: '金周天' })).toBeVisible()
    expect(within(dialog).getByLabelText('模板忽略区域画笔')).toBeVisible()

    await user.click(within(dialog).getByRole('button', { name: '取消' }))
    await waitFor(() => expect(dialog).not.toBeVisible())
    const reopened = await openListenerEditor(user)
    await waitFor(() => expect(api.getBuffListenerTemplate).toHaveBeenCalledTimes(2))
    expect(await within(reopened).findByText('裁剪图标主体')).toBeVisible()
    expect(within(reopened).getByRole('application', { name: '金周天' })).toBeVisible()
  })

  it('shows the listener list and starts all enabled configured items', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    expect(await screen.findByText('已添加 1/8 个，启用项会同时监听。')).toBeVisible()
    expect(screen.getByText('金周天')).toBeVisible()
    const start = screen.getByRole('button', { name: '开始监控' })
    expect(start).toBeEnabled()
    await user.click(start)
    expect(api.startBuffMonitor).toHaveBeenCalledOnce()
  })

  it('updates the enabled state through the listener API', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    await user.click(await screen.findByRole('checkbox', { name: '金周天' }))
    await waitFor(() =>
      expect(api.updateBuffListener).toHaveBeenCalledWith(
        'jinzhoutian',
        '金周天',
        false,
        expect.any(Object)
      )
    )
  })

  it('keeps configuration controls outside the global settings dialog', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    expect(await screen.findByRole('button', { name: '开始监控' })).toBeVisible()
    expect(screen.getByRole('button', { name: '调整悬浮位置' })).toBeVisible()
    const dialog = await openGlobalSettings(user)
    expect(within(dialog).queryByRole('button', { name: '开始监控' })).toBeNull()
    expect(within(dialog).queryByRole('button', { name: '调整悬浮位置' })).toBeNull()
  })

  it('offers TTS Online from the per-listener sound editor', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openListenerEditor(user)
    await user.click(within(dialog).getByRole('button', { name: '前往 TTS Online' }))
    expect(api.openTtsOnline).toHaveBeenCalledOnce()
  })
})
