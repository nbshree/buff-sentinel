import { useEffect, useState } from 'react'

import { WindowTitleBar } from './components/layout/WindowTitleBar'
import { UpdateDialog } from './components/update/UpdateDialog'
import { BuffAssistantPage } from './features/buff-assistant'
import { useBuffAssistantController } from './hooks/useBuffAssistantController'
import { useAppUpdater } from './hooks/useAppUpdater'

function App(): React.JSX.Element {
  const controller = useBuffAssistantController()
  const updater = useAppUpdater(
    controller.state.isMonitoring ? 'Buff 监控正在运行，请先停止监控再安装更新。' : null
  )
  const [appVersion, setAppVersion] = useState<string | null>(null)

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
    void updater.checkOnStartup()
  }, [updater.checkOnStartup])

  return (
    <main className="app-shell">
      <WindowTitleBar
        title="BuffFlow"
        version={appVersion ? `v${appVersion}` : undefined}
        onCheckForUpdate={() => void updater.checkForUpdate()}
        updateBusy={updater.isBusy}
      />
      <section className="app-content" aria-label="Buff 助手">
        <BuffAssistantPage controller={controller} />
      </section>
      <UpdateDialog updater={updater} />
    </main>
  )
}

export default App
