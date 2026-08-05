import {
  Calculator,
  Castle,
  ChevronDown,
  Gamepad2,
  LoaderCircle,
  Palette,
  Radar,
  RefreshCw,
  Sparkles,
  Workflow,
  type LucideIcon
} from 'lucide-react'
import type { RefObject } from 'react'

import type { MacroController } from '../../hooks/useMacroController'
import {
  isWorkspaceView,
  VISIBLE_WORKSPACE_ORDER,
  type WorkspaceView
} from '../../lib/workspace-preference'
import { getThemeDefinition, normalizeAppearance } from '../../themes'
import { Button } from '../ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger
} from '../ui/dropdown-menu'

export type { WorkspaceView } from '../../lib/workspace-preference'

type WorkspaceHeaderProps = {
  controller: MacroController
  activeWorkspace: WorkspaceView
  appVersion: string | null
  themeTriggerRef: RefObject<HTMLButtonElement | null>
  updateTriggerRef: RefObject<HTMLButtonElement | null>
  isCheckingUpdate: boolean
  isSwitchingWorkspace: boolean
  onWorkspaceChange: (workspace: WorkspaceView) => void
  onOpenTheme: () => void
  onCheckForUpdate: () => void
}

const workspaceLabels: Record<
  WorkspaceView,
  { title: string; subtitle: string; menuLabel: string; icon: LucideIcon }
> = {
  macro: {
    title: '宏流程',
    subtitle: '自动化流程管理',
    menuLabel: '宏流程',
    icon: Workflow
  },
  gameRecorder: {
    title: '游戏操作录制',
    subtitle: '相对鼠标与键盘时间轴录制回放',
    menuLabel: '游戏录制',
    icon: Gamepad2
  },
  buffAssistant: {
    title: '金周天 Buff 助手',
    subtitle: '屏幕识别、固定时间轴与悬浮预警',
    menuLabel: 'Buff 助手',
    icon: Radar
  },
  calculator: {
    title: '防守内功评估',
    subtitle: '词条、特性与周天收益分析',
    menuLabel: '防守内功',
    icon: Calculator
  },
  towerCalculator: {
    title: '拆塔内功评估',
    subtitle: '双套抗拆、空拆与周天收益对比',
    menuLabel: '拆塔评估',
    icon: Castle
  }
}

export function WorkspaceHeader({
  controller,
  activeWorkspace,
  appVersion,
  themeTriggerRef,
  updateTriggerRef,
  isCheckingUpdate,
  isSwitchingWorkspace,
  onWorkspaceChange,
  onOpenTheme,
  onCheckForUpdate
}: WorkspaceHeaderProps) {
  const { state } = controller
  const appearance = normalizeAppearance(state.appearance)
  const theme = getThemeDefinition(appearance.themeId)
  const label = workspaceLabels[activeWorkspace]

  function handleWorkspaceChange(workspace: string): void {
    if (!isSwitchingWorkspace && isWorkspaceView(workspace)) onWorkspaceChange(workspace)
  }

  return (
    <header className="workspace-header">
      <div className="workspace-brand">
        <div className="workspace-brand__eyebrow">
          <span className="workspace-brand__author">作者 小踢踢</span>
        </div>
        <h1 id="workspace-title">
          <span>{label.title}</span>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                className="workspace-menu-trigger"
                type="button"
                variant="ghost"
                size="icon-lg"
                aria-label={`切换工作区，当前为${label.menuLabel}`}
                disabled={isSwitchingWorkspace}
              >
                <ChevronDown aria-hidden="true" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" aria-label="工作区">
              <DropdownMenuRadioGroup value={activeWorkspace} onValueChange={handleWorkspaceChange}>
                {VISIBLE_WORKSPACE_ORDER.map((workspace) => {
                  const item = workspaceLabels[workspace]
                  const Icon = item.icon

                  return (
                    <DropdownMenuRadioItem
                      key={workspace}
                      value={workspace}
                      disabled={isSwitchingWorkspace}
                    >
                      <Icon aria-hidden="true" />
                      <span>{item.menuLabel}</span>
                    </DropdownMenuRadioItem>
                  )
                })}
              </DropdownMenuRadioGroup>
            </DropdownMenuContent>
          </DropdownMenu>
        </h1>
        <p>{label.subtitle}</p>
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
