import { screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createRef } from 'react'
import { describe, expect, it, vi } from 'vitest'

import { createMacroController, createMacroState, renderWithUiProviders } from '@/test/test-utils'
import type { ThemeId } from '@/themes'

import { WorkspaceHeader, type WorkspaceView } from './WorkspaceHeader'

function renderHeader(
  themeId: ThemeId,
  onOpenTheme = vi.fn(),
  activeWorkspace: WorkspaceView = 'macro',
  isSwitchingWorkspace = false
) {
  const controller = createMacroController({
    state: createMacroState({ appearance: { themeId, cleanMode: false } })
  })
  const onWorkspaceChange = vi.fn()
  const onCheckForUpdate = vi.fn()

  renderWithUiProviders(
    <WorkspaceHeader
      controller={controller}
      activeWorkspace={activeWorkspace}
      appVersion="1.8.1"
      themeTriggerRef={createRef<HTMLButtonElement>()}
      updateTriggerRef={createRef<HTMLButtonElement>()}
      isCheckingUpdate={false}
      isSwitchingWorkspace={isSwitchingWorkspace}
      onWorkspaceChange={onWorkspaceChange}
      onOpenTheme={onOpenTheme}
      onCheckForUpdate={onCheckForUpdate}
    />
  )

  return { onCheckForUpdate, onOpenTheme, onWorkspaceChange }
}

describe('WorkspaceHeader', () => {
  it('uses 宏流程 as the macro workspace heading', () => {
    renderHeader('longyin')

    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('宏流程')
    expect(screen.getByRole('heading', { level: 1 })).not.toHaveTextContent('shree盒子')
  })

  it('shows the current app version beside the update action', () => {
    renderHeader('longyin')

    expect(screen.getByLabelText('当前版本 v1.8.1')).toHaveTextContent('v1.8.1')
  })

  it.each([
    ['longyin', '主题：龙吟'],
    ['chaoguang', '主题：潮光'],
    ['xuehe', '主题：血河'],
    ['jiuling', '主题：九灵'],
    ['suwen', '主题：素问'],
    ['shenxiang', '主题：神相']
  ] as const)('shows the registered theme name for %s', (themeId, label) => {
    renderHeader(themeId)

    expect(screen.getByRole('button', { name: label })).toBeInTheDocument()
    expect(screen.queryByText('龙吟·霜刃')).not.toBeInTheDocument()
  })

  it('opens the theme dialog from the theme button', async () => {
    const user = userEvent.setup()
    const onOpenTheme = vi.fn()
    renderHeader('longyin', onOpenTheme)

    await user.click(screen.getByRole('button', { name: '主题：龙吟' }))

    expect(onOpenTheme).toHaveBeenCalledTimes(1)
  })

  it('checks for updates only when the update button is clicked', async () => {
    const user = userEvent.setup()
    const { onCheckForUpdate } = renderHeader('longyin')

    expect(onCheckForUpdate).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: '检查更新' }))

    expect(onCheckForUpdate).toHaveBeenCalledTimes(1)
  })

  it('opens the workspace menu from the title arrow', async () => {
    const user = userEvent.setup()
    renderHeader('longyin')

    await user.click(screen.getByRole('button', { name: '切换工作区，当前为宏流程' }))

    expect(screen.getByRole('menu')).toBeInTheDocument()
  })

  it('shows workspaces in the requested order', async () => {
    const user = userEvent.setup()
    renderHeader('longyin')

    await user.click(screen.getByRole('button', { name: '切换工作区，当前为宏流程' }))

    expect(screen.getAllByRole('menuitemradio').map((item) => item.textContent)).toEqual([
      '宏流程',
      'Buff 助手',
      '游戏录制'
    ])
    expect(screen.queryByRole('menuitemradio', { name: '防守内功' })).not.toBeInTheDocument()
    expect(screen.queryByRole('menuitemradio', { name: '拆塔评估' })).not.toBeInTheDocument()
  })

  it('marks the active workspace and switches from a menu item', async () => {
    const user = userEvent.setup()
    const { onWorkspaceChange } = renderHeader('longyin')

    await user.click(screen.getByRole('button', { name: '切换工作区，当前为宏流程' }))

    expect(screen.getByRole('menuitemradio', { name: '宏流程' })).toHaveAttribute(
      'aria-checked',
      'true'
    )
    expect(screen.getByRole('menuitemradio', { name: 'Buff 助手' })).toHaveAttribute(
      'aria-checked',
      'false'
    )

    await user.click(screen.getByRole('menuitemradio', { name: '游戏录制' }))

    expect(onWorkspaceChange).toHaveBeenCalledWith('gameRecorder')
  })

  it('supports keyboard navigation and selection', async () => {
    const user = userEvent.setup()
    const { onWorkspaceChange } = renderHeader('longyin')
    const trigger = screen.getByRole('button', { name: '切换工作区，当前为宏流程' })

    trigger.focus()
    await user.keyboard('{Enter}')
    await user.keyboard('{ArrowDown}{Enter}')

    expect(onWorkspaceChange).toHaveBeenCalledWith('buffAssistant')
    expect(trigger).toHaveFocus()
  })

  it('disables the workspace menu while a switch is pending', () => {
    renderHeader('longyin', vi.fn(), 'macro', true)

    expect(screen.getByRole('button', { name: '切换工作区，当前为宏流程' })).toBeDisabled()
  })

  it('closes with Escape and returns focus to the trigger', async () => {
    const user = userEvent.setup()
    renderHeader('longyin')
    const trigger = screen.getByRole('button', { name: '切换工作区，当前为宏流程' })

    await user.click(trigger)
    await user.keyboard('{Escape}')

    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()
  })

  it('uses the active workspace label in the heading and trigger name', () => {
    renderHeader('longyin', vi.fn(), 'towerCalculator')

    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('拆塔内功评估')
    expect(screen.getByRole('button', { name: '切换工作区，当前为拆塔评估' })).toBeInTheDocument()
  })
})
