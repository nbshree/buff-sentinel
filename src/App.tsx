import { useEffect, useState } from 'react'

import { WindowTitleBar } from './components/layout/WindowTitleBar'
import { BuffAssistantPage } from './features/buff-assistant'
import { useBuffAssistantController } from './hooks/useBuffAssistantController'

function App(): React.JSX.Element {
  const controller = useBuffAssistantController()
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

  return (
    <main className="app-shell">
      <WindowTitleBar title="Buff 哨兵" />
      <header className="app-header">
        <div>
          <h1>Buff 哨兵</h1>
          <p>Buff Sentinel · 屏幕状态识别与提醒</p>
        </div>
        {appVersion ? <span className="app-version">v{appVersion}</span> : null}
      </header>
      <section className="app-content" aria-label="Buff 助手">
        <BuffAssistantPage controller={controller} />
      </section>
    </main>
  )
}

export default App
