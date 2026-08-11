/// <reference types="vite/client" />

import type { BuffSentinelAPI } from './lib/buff-sentinel-api'

declare global {
  interface Window {
    api: BuffSentinelAPI
  }
}
