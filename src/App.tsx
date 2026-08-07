import { lazy, Suspense, useEffect, useRef, useState } from 'react'

import { WindowTitleBar } from './components/layout/WindowTitleBar'
import { WorkspaceHeader } from './components/layout/WorkspaceHeader'
import { FeedbackDialog } from './components/feedback'
import { ControlPanel } from './components/panels/ControlPanel'
import { FlowPanel } from './components/panels/FlowPanel'
import { LogPanel } from './components/panels/LogPanel'
import { ProfilePanel } from './components/panels/ProfilePanel'
import { SettingsPanel } from './components/panels/SettingsPanel'
import { ThemeBackground, ThemeDialog } from './components/theme'
import { UpdateDialog } from './components/update/UpdateDialog'
import { Alert, AlertDescription } from './components/ui/alert'
import { TooltipProvider } from './components/ui/tooltip'
import { GameRecorderPage } from './features/game-recorder'
import { InternalSkillCalculatorPage } from './features/internal-skill-calculator'
import { TowerDemolitionCalculatorPage } from './features/tower-demolition-calculator'
import { useAppUpdater } from './hooks/useAppUpdater'
import { useBuffAssistantController } from './hooks/useBuffAssistantController'
import { useGameRecorderController } from './hooks/useGameRecorderController'
import { useMacroController } from './hooks/useMacroController'
import { getInstallBlockedReason } from './lib/install-blocking'
import type { FeatureAccessStatus } from './lib/macro-api'
import {
  loadWorkspacePreference,
  saveWorkspacePreference,
  type WorkspaceView
} from './lib/workspace-preference'
import { ThemeProvider } from './themes'

const BuffAssistantPage = lazy(() =>
  import('./features/buff-assistant').then((module) => ({ default: module.BuffAssistantPage }))
)

function App(): React.JSX.Element {
  const controller = useMacroController()
  const gameRecorderController = useGameRecorderController(
    controller.state.isRunning || controller.state.isRecording
  )
  const buffAssistantController = useBuffAssistantController()
  const gameActivityBusy = gameRecorderController.state.activity !== 'idle'
  const macroUiController = gameActivityBusy ? { ...controller, isEditingLocked: true } : controller
  const [activeWorkspace, setActiveWorkspace] = useState<WorkspaceView>('buffAssistant')
  const [restrictedWorkspacesUnlocked, setRestrictedWorkspacesUnlocked] = useState(false)
  const [workspaceSwitching, setWorkspaceSwitching] = useState(false)
  const [workspaceSwitchError, setWorkspaceSwitchError] = useState<string | null>(null)
  const [themeDialogOpen, setThemeDialogOpen] = useState(false)
  const [feedbackDialogOpen, setFeedbackDialogOpen] = useState(false)
  const [appVersion, setAppVersion] = useState<string | null>(null)
  const themeTriggerRef = useRef<HTMLButtonElement>(null)
  const feedbackTriggerRef = useRef<HTMLButtonElement>(null)
  const updateTriggerRef = useRef<HTMLButtonElement>(null)
  const installBlockedReason = getInstallBlockedReason({
    macroIsRunning: controller.state.isRunning,
    macroIsRecording: controller.state.isRecording,
    gameActivity: gameRecorderController.state.activity,
    buffActivity: buffAssistantController.state.activity,
    buffIsMonitoring: buffAssistantController.state.isMonitoring,
    gameHasUnsavedChanges:
      gameRecorderController.hasHotkeyChanges ||
      gameRecorderController.hasPlaybackChanges ||
      gameRecorderController.hasNameChanges,
    macroHasUnsavedChanges: controller.hasUnsavedChanges
  })
  const updater = useAppUpdater({ installBlockedReason })

  useEffect(() => {
    void updater.checkOnStartup()
  }, [updater.checkOnStartup])

  useEffect(() => {
    let disposed = false

    void window.api
      .getAppVersion()
      .then((version) => {
        if (!disposed) setAppVersion(version)
      })
      .catch((error: unknown) => {
        if (!disposed) console.error('读取应用版本失败', error)
      })

    return () => {
      disposed = true
    }
  }, [])

  useEffect(() => {
    if (!gameActivityBusy) return
    controller.stopHotkeyCapture()
    controller.closeKeyStepEditor()
    controller.setCapturingPointKeyId(null)
    void window.api.setKeyCapture(false)
  }, [gameActivityBusy])

  async function switchWorkspace(workspace: WorkspaceView): Promise<void> {
    if (workspace === activeWorkspace || workspaceSwitching) return

    setWorkspaceSwitching(true)
    setWorkspaceSwitchError(null)
    controller.stopHotkeyCapture()
    gameRecorderController.stopHotkeyCapture()
    try {
      await window.api.switchWorkspace(workspace)
      setActiveWorkspace(workspace)
      saveWorkspacePreference(workspace)
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error)
      setWorkspaceSwitchError(`无法切换工作区：${message}`)
      console.error('切换工作区失败', error)
    } finally {
      setWorkspaceSwitching(false)
    }
  }

  useEffect(() => {
    let disposed = false

    void window.api
      .getFeatureAccessStatus()
      .then(async (status) => {
        if (disposed) return

        setRestrictedWorkspacesUnlocked(status.restrictedWorkspacesUnlocked)
        const preferredWorkspace = loadWorkspacePreference(status.restrictedWorkspacesUnlocked)
        if (!status.restrictedWorkspacesUnlocked) saveWorkspacePreference(preferredWorkspace)
        if (preferredWorkspace !== 'buffAssistant') await switchWorkspace(preferredWorkspace)
      })
      .catch((error: unknown) => {
        if (disposed) return

        const message = error instanceof Error ? error.message : String(error)
        setWorkspaceSwitchError(`无法读取功能访问状态：${message}`)
        console.error('读取功能访问状态失败', error)
      })

    return () => {
      disposed = true
    }
  }, [])

  async function submitFeedback(content: string): Promise<FeatureAccessStatus> {
    const status = await window.api.submitFeedback(content)
    setRestrictedWorkspacesUnlocked(status.restrictedWorkspacesUnlocked)
    if (!status.restrictedWorkspacesUnlocked) {
      setActiveWorkspace('buffAssistant')
      saveWorkspacePreference('buffAssistant')
    }
    return status
  }

  return (
    <ThemeProvider appearance={controller.state.appearance}>
      <TooltipProvider delayDuration={300} skipDelayDuration={100}>
        <main className="theme-app-shell app-shell">
          <ThemeBackground
            onAssetError={(asset, source) => console.warn(`主题素材加载失败：${asset}`, source)}
          />
          <div className="theme-content-layer app-frame">
            <WindowTitleBar />
            <div className="workspace">
              <WorkspaceHeader
                controller={controller}
                activeWorkspace={activeWorkspace}
                appVersion={appVersion}
                themeTriggerRef={themeTriggerRef}
                updateTriggerRef={updateTriggerRef}
                feedbackTriggerRef={feedbackTriggerRef}
                restrictedWorkspacesUnlocked={restrictedWorkspacesUnlocked}
                isCheckingUpdate={updater.status === 'checking'}
                isSwitchingWorkspace={workspaceSwitching}
                onWorkspaceChange={(workspace) => void switchWorkspace(workspace)}
                onOpenTheme={() => setThemeDialogOpen(true)}
                onOpenFeedback={() => setFeedbackDialogOpen(true)}
                onCheckForUpdate={() => void updater.checkForUpdate()}
              />
              {workspaceSwitchError ? (
                <Alert className="workspace-switch-error" variant="destructive">
                  <AlertDescription>{workspaceSwitchError}</AlertDescription>
                </Alert>
              ) : null}
              <section
                className="workspace-view"
                id="buff-assistant-workspace"
                role="region"
                aria-labelledby="workspace-title"
                hidden={activeWorkspace !== 'buffAssistant'}
              >
                <Suspense fallback={<div className="workspace-loading">正在载入 Buff 助手…</div>}>
                  <BuffAssistantPage controller={buffAssistantController} />
                </Suspense>
              </section>
              <section
                className="workspace-view"
                id="macro-workspace"
                role="region"
                aria-labelledby="workspace-title"
                hidden={activeWorkspace !== 'macro'}
              >
                <section className="workspace-grid">
                  <aside className="sidebar" aria-label="宏控制与配置">
                    <ControlPanel controller={macroUiController} />
                    <ProfilePanel controller={macroUiController} />
                    <SettingsPanel controller={macroUiController} />
                  </aside>
                  <section className="main-workspace" aria-label="宏流程与执行日志">
                    <FlowPanel controller={macroUiController} />
                    <LogPanel controller={controller} />
                  </section>
                </section>
              </section>
              <section
                className="workspace-view"
                id="game-recorder-workspace"
                role="region"
                aria-labelledby="workspace-title"
                hidden={activeWorkspace !== 'gameRecorder'}
              >
                <GameRecorderPage controller={gameRecorderController} />
              </section>
              <section
                className="workspace-view"
                id="calculator-workspace"
                role="region"
                aria-labelledby="workspace-title"
                hidden={activeWorkspace !== 'calculator'}
              >
                <InternalSkillCalculatorPage active={activeWorkspace === 'calculator'} />
              </section>
              <section
                className="workspace-view"
                id="tower-calculator-workspace"
                role="region"
                aria-labelledby="workspace-title"
                hidden={activeWorkspace !== 'towerCalculator'}
              >
                <TowerDemolitionCalculatorPage />
              </section>
            </div>
          </div>

          <ThemeDialog
            appearance={controller.state.appearance}
            open={themeDialogOpen}
            returnFocusRef={themeTriggerRef}
            onApply={(appearance) => controller.updateAppearance(appearance)}
            onOpenChange={setThemeDialogOpen}
          />
          <FeedbackDialog
            open={feedbackDialogOpen}
            restrictedWorkspacesUnlocked={restrictedWorkspacesUnlocked}
            returnFocusRef={feedbackTriggerRef}
            onOpenChange={setFeedbackDialogOpen}
            onSubmit={submitFeedback}
          />
          <UpdateDialog updater={updater} returnFocusRef={updateTriggerRef} />
        </main>
      </TooltipProvider>
    </ThemeProvider>
  )
}

export default App
