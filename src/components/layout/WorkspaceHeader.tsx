import {
  Calculator,
  Castle,
  Gamepad2,
  LoaderCircle,
  Palette,
  Radar,
  RefreshCw,
  Sparkles,
  Workflow
} from 'lucide-react'
import type { KeyboardEvent, RefObject } from 'react'

import type { MacroController } from '../../hooks/useMacroController'
import { getThemeDefinition, normalizeAppearance } from '../../themes'
import { Button } from '../ui/button'

type WorkspaceHeaderProps = {
  controller: MacroController
  activeWorkspace: WorkspaceView
  appVersion: string | null
  themeTriggerRef: RefObject<HTMLButtonElement | null>
  updateTriggerRef: RefObject<HTMLButtonElement | null>
  isCheckingUpdate: boolean
  onWorkspaceChange: (workspace: WorkspaceView) => void
  onOpenTheme: () => void
  onCheckForUpdate: () => void
}

export type WorkspaceView =
  'macro' | 'gameRecorder' | 'buffAssistant' | 'calculator' | 'towerCalculator'

const workspaceOrder: readonly WorkspaceView[] = [
  'macro',
  'gameRecorder',
  'buffAssistant',
  'calculator',
  'towerCalculator'
]
const workspaceTabIds: Record<WorkspaceView, string> = {
  macro: 'workspace-tab-macro',
  gameRecorder: 'workspace-tab-game-recorder',
  buffAssistant: 'workspace-tab-buff-assistant',
  calculator: 'workspace-tab-calculator',
  towerCalculator: 'workspace-tab-tower-calculator'
}

const workspaceLabels: Record<WorkspaceView, { title: string; subtitle: string }> = {
  macro: {
    title: '自动点击流程台',
    subtitle: '自动化流程管理'
  },
  gameRecorder: {
    title: '游戏操作录制',
    subtitle: '相对鼠标与键盘时间轴录制回放'
  },
  buffAssistant: {
    title: '金周天 Buff 助手',
    subtitle: '屏幕识别、固定时间轴与悬浮预警'
  },
  calculator: {
    title: '防守内功评估',
    subtitle: '词条、特性与周天收益分析'
  },
  towerCalculator: {
    title: '拆塔内功评估',
    subtitle: '双套抗拆、空拆与周天收益对比'
  }
}

export function WorkspaceHeader({
  controller,
  activeWorkspace,
  appVersion,
  themeTriggerRef,
  updateTriggerRef,
  isCheckingUpdate,
  onWorkspaceChange,
  onOpenTheme,
  onCheckForUpdate
}: WorkspaceHeaderProps) {
  const { state } = controller
  const appearance = normalizeAppearance(state.appearance)
  const theme = getThemeDefinition(appearance.themeId)
  const label = workspaceLabels[activeWorkspace]

  function handleWorkspaceKeyDown(event: KeyboardEvent<HTMLButtonElement>): void {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return

    event.preventDefault()
    const currentIndex = workspaceOrder.indexOf(activeWorkspace)
    const nextWorkspace =
      event.key === 'Home'
        ? workspaceOrder[0]
        : event.key === 'End'
          ? workspaceOrder[workspaceOrder.length - 1]
          : workspaceOrder[
              (currentIndex + (event.key === 'ArrowRight' ? 1 : -1) + workspaceOrder.length) %
                workspaceOrder.length
            ]

    onWorkspaceChange(nextWorkspace)
    document.getElementById(workspaceTabIds[nextWorkspace])?.focus()
  }

  return (
    <header className="workspace-header">
      <div className="workspace-brand">
        <div className="workspace-brand__eyebrow">
          <span>Shree Macro Flow</span>
          <span className="workspace-brand__author">作者 小踢踢</span>
        </div>
        <h1>{label.title}</h1>
        <p>{label.subtitle}</p>
      </div>

      <div className="workspace-switcher" role="tablist" aria-label="工作区">
        <Button
          className="workspace-switcher__tab"
          data-active={activeWorkspace === 'macro'}
          id="workspace-tab-macro"
          type="button"
          role="tab"
          variant="ghost"
          aria-controls="macro-workspace"
          aria-selected={activeWorkspace === 'macro'}
          tabIndex={activeWorkspace === 'macro' ? 0 : -1}
          onKeyDown={handleWorkspaceKeyDown}
          onClick={() => onWorkspaceChange('macro')}
        >
          <Workflow aria-hidden="true" />
          <span>宏流程</span>
        </Button>
        <Button
          className="workspace-switcher__tab"
          data-active={activeWorkspace === 'gameRecorder'}
          id="workspace-tab-game-recorder"
          type="button"
          role="tab"
          variant="ghost"
          aria-controls="game-recorder-workspace"
          aria-selected={activeWorkspace === 'gameRecorder'}
          tabIndex={activeWorkspace === 'gameRecorder' ? 0 : -1}
          onKeyDown={handleWorkspaceKeyDown}
          onClick={() => onWorkspaceChange('gameRecorder')}
        >
          <Gamepad2 aria-hidden="true" />
          <span>游戏录制</span>
        </Button>
        <Button
          className="workspace-switcher__tab"
          data-active={activeWorkspace === 'buffAssistant'}
          id="workspace-tab-buff-assistant"
          type="button"
          role="tab"
          variant="ghost"
          aria-controls="buff-assistant-workspace"
          aria-selected={activeWorkspace === 'buffAssistant'}
          tabIndex={activeWorkspace === 'buffAssistant' ? 0 : -1}
          onKeyDown={handleWorkspaceKeyDown}
          onClick={() => onWorkspaceChange('buffAssistant')}
        >
          <Radar aria-hidden="true" />
          <span>Buff 助手</span>
        </Button>
        <Button
          className="workspace-switcher__tab"
          data-active={activeWorkspace === 'calculator'}
          id="workspace-tab-calculator"
          type="button"
          role="tab"
          variant="ghost"
          aria-controls="calculator-workspace"
          aria-selected={activeWorkspace === 'calculator'}
          tabIndex={activeWorkspace === 'calculator' ? 0 : -1}
          onKeyDown={handleWorkspaceKeyDown}
          onClick={() => onWorkspaceChange('calculator')}
        >
          <Calculator aria-hidden="true" />
          <span>防守内功</span>
        </Button>
        <Button
          className="workspace-switcher__tab"
          data-active={activeWorkspace === 'towerCalculator'}
          id="workspace-tab-tower-calculator"
          type="button"
          role="tab"
          variant="ghost"
          aria-controls="tower-calculator-workspace"
          aria-selected={activeWorkspace === 'towerCalculator'}
          tabIndex={activeWorkspace === 'towerCalculator' ? 0 : -1}
          onKeyDown={handleWorkspaceKeyDown}
          onClick={() => onWorkspaceChange('towerCalculator')}
        >
          <Castle aria-hidden="true" />
          <span>拆塔评估</span>
        </Button>
      </div>

      <div className="workspace-header__actions">
        <span className="app-version" aria-label={`当前版本 v${appVersion ?? '未知'}`}>
          v{appVersion ?? '—'}
        </span>
        <Button
          aria-label={isCheckingUpdate ? '正在检查更新' : '检查更新'}
          className="update-trigger rounded-full"
          disabled={isCheckingUpdate}
          ref={updateTriggerRef}
          type="button"
          variant="outline"
          onClick={onCheckForUpdate}
        >
          {isCheckingUpdate ? (
            <LoaderCircle aria-hidden="true" className="update-trigger__spinner" size={17} />
          ) : (
            <RefreshCw aria-hidden="true" size={17} />
          )}
          <span>{isCheckingUpdate ? '检查中' : '检查更新'}</span>
        </Button>
        <Button
          className="theme-trigger rounded-full"
          ref={themeTriggerRef}
          type="button"
          variant="outline"
          onClick={onOpenTheme}
        >
          {appearance.cleanMode ? (
            <Palette aria-hidden="true" size={17} />
          ) : (
            <Sparkles aria-hidden="true" size={17} />
          )}
          <span>主题：{theme.name}</span>
        </Button>
      </div>
    </header>
  )
}
