import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { useBuffAssistantController } from '@/hooks/useBuffAssistantController'
import type { BuffListenerConfig, NormalizedRect } from '@/lib/buff-sentinel-api'
import { createBuffSentinelApi, installBuffSentinelApi } from '@/test/test-utils'

import { BuffAssistantPage } from './BuffAssistantPage'

function BuffAssistantHarness() {
  const controller = useBuffAssistantController()
  return <BuffAssistantPage controller={controller} />
}

async function createListenerApi(hideInOverlay = false) {
  const baseApi = createBuffSentinelApi()
  const baseState = await baseApi.getBuffAssistantState()
  const api = createBuffSentinelApi({
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
          hideInOverlay,
          template: { id: 'template', width: 32, height: 32 },
          settings: {
            cycleMs: 20_000,
            deadlineGraceMs: 1500,
            matchMode: 'pixel',
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
  api.listBuffCaptureWindows.mockResolvedValue([
    {
      id: '1',
      processName: 'game.exe',
      windowTitle: 'Game',
      className: 'GameWindow',
      width: 1920,
      height: 1080
    }
  ])
  api.captureBuffPreview.mockResolvedValue({
    dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
    width: 1920,
    height: 1080,
    target: {
      processName: 'game.exe',
      windowTitle: 'Game',
      className: 'GameWindow',
      referenceWidth: 1920,
      referenceHeight: 1080
    }
  })
  return api
}

async function captureConfiguredPreview(user: ReturnType<typeof userEvent.setup>) {
  const previewButton = await screen.findByRole('button', { name: '捕获预览' })
  await waitFor(() => expect(previewButton).toBeEnabled())
  await user.click(previewButton)
}

async function openGlobalSettings(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('button', { name: '设置' }))
  return screen.findByRole('dialog', { name: '设置' })
}

async function openListenerEditor(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole('button', { name: '编辑' }))
  return screen.findByRole('dialog', { name: '编辑监听图标' })
}

/**
 * jsdom has neither image loading nor a canvas implementation, so the capture
 * preview would never be cropped into a template source. Both are stubbed so
 * the add-listener menu becomes reachable in tests.
 */
function stubCroppedPreview(): () => void {
  const canvasPrototype = HTMLCanvasElement.prototype as unknown as {
    getContext: (...args: unknown[]) => unknown
    toDataURL: (...args: unknown[]) => string
  }
  const originalImage = globalThis.Image
  const originalGetContext = canvasPrototype.getContext
  const originalToDataURL = canvasPrototype.toDataURL

  class ImmediateImage {
    decoding = 'async'
    naturalWidth = 1920
    naturalHeight = 1080
    onload: (() => void) | null = null
    onerror: (() => void) | null = null

    set src(_value: string) {
      this.onload?.()
    }
  }

  Object.defineProperty(globalThis, 'Image', { configurable: true, value: ImmediateImage })
  canvasPrototype.getContext = () => ({ drawImage() {} })
  canvasPrototype.toDataURL = () => 'data:image/png;base64,Y3V0'

  return () => {
    Object.defineProperty(globalThis, 'Image', { configurable: true, value: originalImage })
    canvasPrototype.getContext = originalGetContext
    canvasPrototype.toDataURL = originalToDataURL
  }
}

function listenerFixture(
  id: string,
  name: string,
  kind: BuffListenerConfig['kind']
): BuffListenerConfig {
  return {
    id,
    name,
    enabled: true,
    hideInOverlay: false,
    kind,
    template: { id: `template-${id}`, width: 32, height: 32 },
    settings: {
      cycleMs: 20_000,
      deadlineGraceMs: 1500,
      skillDurationMs: 10_000,
      matchMode: 'pixel',
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
}

async function createTwoKindApi(skillSearchRegion: NormalizedRect | null) {
  const baseApi = createBuffSentinelApi()
  const baseState = await baseApi.getBuffAssistantState()
  const target = {
    processName: 'game.exe',
    windowTitle: 'Game',
    className: 'GameWindow',
    referenceWidth: 1920,
    referenceHeight: 1080
  }
  const api = createBuffSentinelApi({
    ...baseState,
    config: {
      ...baseState.config,
      target,
      searchRegion: { x: 0.5, y: 0, width: 0.4, height: 0.2 },
      skillSearchRegion,
      listeners: [
        listenerFixture('jinzhoutian', '金周天', 'cycle'),
        listenerFixture('skill-1', '疾风步', 'skillCountdown')
      ]
    },
    listeners: [
      {
        id: 'jinzhoutian',
        activity: 'stopped',
        expectedAtUnixMs: null,
        lastConfidence: 0,
        lastError: null
      },
      {
        id: 'skill-1',
        activity: 'stopped',
        expectedAtUnixMs: null,
        lastConfidence: 0,
        lastError: null
      }
    ]
  })
  api.listBuffCaptureWindows.mockResolvedValue([
    {
      id: '1',
      processName: 'game.exe',
      windowTitle: 'Game',
      className: 'GameWindow',
      width: 1920,
      height: 1080
    }
  ])
  api.captureBuffPreview.mockResolvedValue({
    dataUrl: 'data:image/png;base64,iVBORw0KGgo=',
    width: 1920,
    height: 1080,
    target
  })
  return api
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

  it('lists, tests and saves a global audio output device', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openGlobalSettings(user)
    await waitFor(() => expect(api.listBuffAudioOutputDevices).toHaveBeenCalledOnce())
    const output = within(dialog).getByRole('combobox', { name: '播报输出设备' })
    await user.click(output)
    await user.click(await screen.findByRole('option', { name: '耳机' }))
    await user.click(within(dialog).getByRole('button', { name: '测试声音' }))

    expect(api.testBuffAudioOutput).toHaveBeenCalledWith('wasapi:headphones')

    await user.click(within(dialog).getByRole('button', { name: '保存设置' }))
    await waitFor(() =>
      expect(api.updateBuffAssistantSettings).toHaveBeenCalledWith(
        expect.objectContaining({ audioOutputDeviceId: 'wasapi:headphones' })
      )
    )
  })

  it('refreshes audio outputs and reports enumeration failures', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    api.listBuffAudioOutputDevices.mockRejectedValue(new Error('无法读取声音输出设备'))
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openGlobalSettings(user)
    expect(await within(dialog).findByText('无法读取声音输出设备')).toBeVisible()
    await user.click(within(dialog).getByRole('button', { name: '刷新播报输出设备' }))
    await waitFor(() => expect(api.listBuffAudioOutputDevices).toHaveBeenCalledTimes(2))
  })

  it('keeps an unavailable saved audio output and shows the fallback', async () => {
    const user = userEvent.setup()
    const baseApi = await createListenerApi()
    const baseState = await baseApi.getBuffAssistantState()
    const api = createBuffSentinelApi({
      ...baseState,
      config: {
        ...baseState.config,
        settings: {
          ...baseState.config.settings,
          audioOutputDeviceId: 'wasapi:missing-headset'
        }
      }
    })
    api.listBuffAudioOutputDevices.mockResolvedValue([
      { id: 'wasapi:speakers', name: '扬声器', isDefault: true }
    ])
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openGlobalSettings(user)
    expect(
      await within(dialog).findByText('已选设备当前不可用，播报会临时使用系统默认设备。')
    ).toBeVisible()
    expect(within(dialog).getByRole('button', { name: '测试声音' })).toBeDisabled()
  })

  it('records and saves a global monitor hotkey', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openGlobalSettings(user)
    const hotkey = within(dialog).getByRole('textbox', { name: '监控热键' })
    const hotkeyHelp = within(dialog).getByRole('button', { name: '查看监控热键说明' })
    expect(hotkey).toHaveValue('')
    expect(hotkeyHelp).toBeVisible()
    await user.hover(hotkeyHelp)
    expect(await screen.findByRole('tooltip')).toHaveTextContent(
      '切换开始 / 停止监控；应用最小化到托盘或游戏位于前台时也能切换监控。'
    )

    await user.click(hotkey)
    await user.keyboard('{Control>}{Shift>}k{/Shift}{/Control}')
    expect(hotkey).toHaveValue('Ctrl + Shift + K')
    await user.click(within(dialog).getByRole('button', { name: '保存设置' }))

    await waitFor(() =>
      expect(api.updateBuffAssistantSettings).toHaveBeenCalledWith(
        expect.objectContaining({ monitorHotkey: 'Ctrl+Shift+K' })
      )
    )
  })

  it('clears the monitor hotkey to disable it', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openGlobalSettings(user)
    const hotkey = within(dialog).getByRole('textbox', { name: '监控热键' })
    await user.click(hotkey)
    await user.keyboard('{Control>}{Shift>}k{/Shift}{/Control}')
    await user.click(within(dialog).getByRole('button', { name: '清空' }))
    expect(hotkey).toHaveValue('')
    await user.click(within(dialog).getByRole('button', { name: '保存设置' }))

    await waitFor(() =>
      expect(api.updateBuffAssistantSettings).toHaveBeenCalledWith(
        expect.objectContaining({ monitorHotkey: null })
      )
    )
  })

  it('keeps settings open and shows a hotkey registration failure', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    api.updateBuffAssistantSettings.mockRejectedValueOnce(
      new Error('监控热键注册失败：Ctrl+Alt+F10 可能已被其他程序占用')
    )
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openGlobalSettings(user)
    const hotkey = within(dialog).getByRole('textbox', { name: '监控热键' })
    await user.click(hotkey)
    await user.keyboard('{Control>}{Alt>}{F10}{/Alt}{/Control}')
    await user.click(within(dialog).getByRole('button', { name: '保存设置' }))

    expect(await within(dialog).findByText(/监控热键注册失败：Ctrl\+Alt\+F10/)).toBeVisible()
    expect(dialog).toBeVisible()
  })

  it('edits recognition, timing and sound settings per listener', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openListenerEditor(user)
    expect(within(dialog).getByRole('spinbutton', { name: /触发宽限期/ })).toHaveValue(1500)
    expect(within(dialog).getByRole('checkbox', { name: '真实触发确认音' })).toBeChecked()
    expect(within(dialog).getByRole('checkbox', { name: '隐藏浮窗显示' })).not.toBeChecked()
    await user.clear(within(dialog).getByRole('spinbutton', { name: '周期（秒）' }))
    await user.type(within(dialog).getByRole('spinbutton', { name: '周期（秒）' }), '30')
    await user.click(within(dialog).getByRole('button', { name: '保存监听项' }))

    await waitFor(() =>
      expect(api.updateBuffListener).toHaveBeenCalledWith(
        'jinzhoutian',
        '金周天',
        true,
        false,
        expect.objectContaining({ cycleMs: 30_000 })
      )
    )
  })

  it('edits whether a listener is shown in the overlay', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi(true)
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openListenerEditor(user)
    const hideInOverlay = within(dialog).getByRole('checkbox', { name: '隐藏浮窗显示' })
    expect(hideInOverlay).toBeChecked()
    expect(within(dialog).getByText('仍会继续监听和播放提示音，仅不显示在悬浮窗中。')).toBeVisible()

    await user.click(hideInOverlay)
    await user.click(within(dialog).getByRole('button', { name: '保存监听项' }))

    await waitFor(() =>
      expect(api.updateBuffListener).toHaveBeenCalledWith(
        'jinzhoutian',
        '金周天',
        true,
        false,
        expect.any(Object)
      )
    )
  })

  it('switches recognition modes with smart default thresholds', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openListenerEditor(user)
    const mode = within(dialog).getByRole('combobox', { name: '识别模式' })
    const threshold = within(dialog).getByRole('spinbutton', { name: '匹配阈值' })
    expect(mode).toHaveTextContent('像素图标')
    expect(threshold).toHaveValue(0.95)
    expect(within(dialog).getByRole('button', { name: '查看识别模式说明' })).toBeVisible()

    await user.click(mode)
    await user.click(await screen.findByRole('option', { name: '亮色文字' }))
    expect(threshold).toHaveValue(0.84)

    await user.click(mode)
    await user.click(await screen.findByRole('option', { name: '像素图标' }))
    expect(threshold).toHaveValue(0.95)
  })

  it('preserves a customized threshold when switching recognition modes', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openListenerEditor(user)
    const mode = within(dialog).getByRole('combobox', { name: '识别模式' })
    const threshold = within(dialog).getByRole('spinbutton', { name: '匹配阈值' })
    await user.clear(threshold)
    await user.type(threshold, '0.9')
    await user.click(mode)
    await user.click(await screen.findByRole('option', { name: '亮色文字' }))

    expect(threshold).toHaveValue(0.9)
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
    expect(start).toBeDisabled()
    await captureConfiguredPreview(user)
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

  it('switches the live overlay preview while adjusting its position', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    await user.click(await screen.findByRole('button', { name: '调整悬浮位置' }))
    expect(api.setBuffOverlayEditMode).toHaveBeenCalledWith(true)

    const previewSelect = await screen.findByRole('combobox', { name: '悬浮窗预览状态' })
    expect(previewSelect).toHaveTextContent('倒计时')
    await user.click(previewSelect)
    await user.click(await screen.findByRole('option', { name: '等待确认' }))

    await waitFor(() =>
      expect(api.setBuffOverlayPreviewMode).toHaveBeenCalledWith('confirming')
    )
  })

  it('requires saving the overlay position before monitoring can start', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const editButton = await screen.findByRole('button', { name: '调整悬浮位置' })
    const startButton = screen.getByRole('button', { name: '开始监控' })
    expect(startButton).toBeDisabled()
    await captureConfiguredPreview(user)
    expect(startButton).toBeEnabled()

    await user.click(editButton)
    expect(startButton).toBeDisabled()
    expect(startButton).toHaveAttribute('title', '请先保存悬浮位置')

    await user.click(screen.getByRole('button', { name: '保存悬浮位置' }))
    expect(startButton).toBeEnabled()
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

  it('offers both listener mechanisms from the add button menu', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    const restorePreview = stubCroppedPreview()

    try {
      render(<BuffAssistantHarness />)
      await captureConfiguredPreview(user)

      const addButton = await screen.findByRole('button', { name: '添加监听图标' })
      await waitFor(() => expect(addButton).toBeEnabled())
      await user.click(addButton)

      expect(await screen.findByRole('menuitem', { name: /周期提醒/ })).toBeVisible()
      expect(screen.getByRole('menuitem', { name: /技能倒计时/ })).toBeVisible()
    } finally {
      restorePreview()
    }
  })

  it('adds a skill countdown listener without any sound settings', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    const restorePreview = stubCroppedPreview()

    try {
      render(<BuffAssistantHarness />)
      await captureConfiguredPreview(user)

      const addButton = await screen.findByRole('button', { name: '添加监听图标' })
      await waitFor(() => expect(addButton).toBeEnabled())
      await user.click(addButton)
      await user.click(await screen.findByRole('menuitem', { name: /技能倒计时/ }))

      const dialog = await screen.findByRole('dialog', { name: '添加技能倒计时' })
      expect(within(dialog).getByRole('textbox', { name: '类型' })).toHaveValue('技能倒计时')
      expect(within(dialog).getByRole('spinbutton', { name: '技能持续时间（秒）' })).toHaveValue(10)
      expect(within(dialog).getByRole('spinbutton', { name: '匹配阈值' })).toBeVisible()
      expect(within(dialog).queryByRole('spinbutton', { name: '周期（秒）' })).toBeNull()
      expect(within(dialog).queryByRole('spinbutton', { name: /触发宽限期/ })).toBeNull()
      expect(within(dialog).queryByRole('checkbox', { name: '真实触发确认音' })).toBeNull()
      expect(within(dialog).queryByRole('button', { name: '前往 TTS Online' })).toBeNull()
      expect(
        within(dialog).getByText('名称、模板与识别参数仅作用于当前项；技能倒计时不会播放提示音。')
      ).toBeVisible()
    } finally {
      restorePreview()
    }
  })

  it('keeps the listener mechanism read-only while editing', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const dialog = await openListenerEditor(user)
    const kind = within(dialog).getByRole('textbox', { name: '类型' })

    expect(kind).toHaveValue('周期提醒')
    expect(kind).toHaveAttribute('readonly')
    expect(within(dialog).getByRole('spinbutton', { name: '周期（秒）' })).toBeVisible()
  })

  it('offers one search region block per listener mechanism', async () => {
    const user = userEvent.setup()
    const api = await createTwoKindApi(null)
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    await captureConfiguredPreview(user)

    expect(await screen.findByText('框选 Buff 栏搜索区域')).toBeVisible()
    expect(screen.getByText('框选技能图标搜索区域')).toBeVisible()
    expect(screen.getByRole('application', { name: 'Buff 搜索区域' })).toBeVisible()
    expect(screen.getByRole('application', { name: '技能搜索区域' })).toBeVisible()
  })

  it('persists the search region of the mechanism that was framed', async () => {
    const user = userEvent.setup()
    const api = await createTwoKindApi(null)
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)
    await captureConfiguredPreview(user)

    const skillCanvas = await screen.findByRole('application', { name: '技能搜索区域' })
    vi.spyOn(skillCanvas, 'getBoundingClientRect').mockReturnValue({
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
    fireEvent.pointerDown(skillCanvas, { button: 0, clientX: 100, clientY: 50, detail: 1 })
    fireEvent.pointerMove(skillCanvas, { clientX: 400, clientY: 250 })
    fireEvent.pointerUp(skillCanvas, { clientX: 400, clientY: 250, detail: 1 })

    await waitFor(() =>
      expect(api.updateBuffSearchRegion).toHaveBeenCalledWith('skillCountdown', {
        x: 0.1,
        y: 0.1,
        width: 0.3,
        height: 0.4
      })
    )
  })

  it('does not require the skill search region for cycle reminders', async () => {
    const user = userEvent.setup()
    const api = await createListenerApi()
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const start = await screen.findByRole('button', { name: '开始监控' })
    await captureConfiguredPreview(user)

    expect(start).toBeEnabled()
  })

  it('requires the skill search region once a skill countdown listener is enabled', async () => {
    const user = userEvent.setup()
    const api = await createTwoKindApi(null)
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const start = await screen.findByRole('button', { name: '开始监控' })
    await captureConfiguredPreview(user)

    expect(start).toBeDisabled()
  })

  it('enables monitoring when the skill search region is saved', async () => {
    const user = userEvent.setup()
    const api = await createTwoKindApi({ x: 0.1, y: 0.7, width: 0.12, height: 0.12 })
    installBuffSentinelApi(api)
    render(<BuffAssistantHarness />)

    const start = await screen.findByRole('button', { name: '开始监控' })
    await captureConfiguredPreview(user)

    expect(start).toBeEnabled()
  })
})
