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
  activeWorkspace: WorkspaceView = 'macro'
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
      onWorkspaceChange={onWorkspaceChange}
      onOpenTheme={onOpenTheme}
      onCheckForUpdate={onCheckForUpdate}
    />
  )

  return { onCheckForUpdate, onOpenTheme, onWorkspaceChange }
}

describe('WorkspaceHeader', () => {
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

  it('switches to the calculator workspace from the top-level tabs', async () => {
    const user = userEvent.setup()
    const { onWorkspaceChange } = renderHeader('longyin')

    await user.click(screen.getByRole('tab', { name: '防守内功' }))

    expect(onWorkspaceChange).toHaveBeenCalledWith('calculator')
  })

  it('switches to the game recorder workspace from the top-level tabs', async () => {
    const user = userEvent.setup()
    const { onWorkspaceChange } = renderHeader('longyin')

    await user.click(screen.getByRole('tab', { name: '游戏录制' }))

    expect(onWorkspaceChange).toHaveBeenCalledWith('gameRecorder')
  })

  it('switches to the tower calculator workspace from the top-level tabs', async () => {
    const user = userEvent.setup()
    const { onWorkspaceChange } = renderHeader('longyin')

    await user.click(screen.getByRole('tab', { name: '拆塔评估' }))

    expect(onWorkspaceChange).toHaveBeenCalledWith('towerCalculator')
  })

  it('supports arrow-key navigation between workspace tabs', async () => {
    const user = userEvent.setup()
    const { onWorkspaceChange } = renderHeader('longyin')

    const macroTab = screen.getByRole('tab', { name: '宏流程' })
    macroTab.focus()
    await user.keyboard('{ArrowRight}')

    expect(onWorkspaceChange).toHaveBeenCalledWith('gameRecorder')
    expect(screen.getByRole('tab', { name: '游戏录制' })).toHaveFocus()
  })

  it('cycles through four workspace tabs with the arrow keys', async () => {
    const user = userEvent.setup()
    const { onWorkspaceChange } = renderHeader('longyin', vi.fn(), 'towerCalculator')

    const towerTab = screen.getByRole('tab', { name: '拆塔评估' })
    towerTab.focus()
    await user.keyboard('{ArrowRight}')

    expect(onWorkspaceChange).toHaveBeenLastCalledWith('macro')
    expect(screen.getByRole('tab', { name: '宏流程' })).toHaveFocus()

    towerTab.focus()
    await user.keyboard('{ArrowLeft}')

    expect(onWorkspaceChange).toHaveBeenLastCalledWith('calculator')
    expect(screen.getByRole('tab', { name: '防守内功' })).toHaveFocus()
  })

  it('moves to the first and last workspace tabs with Home and End', async () => {
    const user = userEvent.setup()
    const { onWorkspaceChange } = renderHeader('longyin', vi.fn(), 'calculator')
    const calculatorTab = screen.getByRole('tab', { name: '防守内功' })

    calculatorTab.focus()
    await user.keyboard('{Home}')

    expect(onWorkspaceChange).toHaveBeenLastCalledWith('macro')
    expect(screen.getByRole('tab', { name: '宏流程' })).toHaveFocus()

    calculatorTab.focus()
    await user.keyboard('{End}')

    expect(onWorkspaceChange).toHaveBeenLastCalledWith('towerCalculator')
    expect(screen.getByRole('tab', { name: '拆塔评估' })).toHaveFocus()
  })

  it('exposes a roving tab stop and matching panel relationships', () => {
    renderHeader('longyin', vi.fn(), 'towerCalculator')

    const tabs = screen.getAllByRole('tab')
    expect(tabs).toHaveLength(5)
    expect(tabs.map((tab) => tab.getAttribute('aria-controls'))).toEqual([
      'macro-workspace',
      'game-recorder-workspace',
      'buff-assistant-workspace',
      'calculator-workspace',
      'tower-calculator-workspace'
    ])
    expect(screen.getByRole('tab', { name: '拆塔评估' })).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('tab', { name: '拆塔评估' })).toHaveAttribute('tabindex', '0')
    expect(screen.getByRole('tab', { name: '宏流程' })).toHaveAttribute('tabindex', '-1')
    expect(screen.getByRole('tab', { name: '游戏录制' })).toHaveAttribute('tabindex', '-1')
    expect(screen.getByRole('tab', { name: 'Buff 助手' })).toHaveAttribute('tabindex', '-1')
    expect(screen.getByRole('tab', { name: '防守内功' })).toHaveAttribute('tabindex', '-1')
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('拆塔内功评估')
  })
})
