import './assets/main.css'
import './lib/buff-sentinel-api'

import { getCurrentWindow } from '@tauri-apps/api/window'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

async function renderApp(): Promise<void> {
  const currentWindow = getCurrentWindow()
  const Component =
    currentWindow.label === 'buff-overlay'
      ? (await import('./features/buff-assistant/BuffOverlayApp')).BuffOverlayApp
      : (await import('./App')).default

  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <Component />
    </StrictMode>
  )
}

void renderApp()
