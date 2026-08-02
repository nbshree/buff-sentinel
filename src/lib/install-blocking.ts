import type { BuffAssistantActivity, GameRecorderActivity } from './macro-api'

type InstallBlockingState = {
  macroIsRunning: boolean
  macroIsRecording: boolean
  gameActivity: GameRecorderActivity
  buffActivity: BuffAssistantActivity
  buffIsMonitoring: boolean
  gameHasUnsavedChanges: boolean
  macroHasUnsavedChanges: boolean
}

export function getInstallBlockedReason({
  macroIsRunning,
  macroIsRecording,
  gameActivity,
  buffActivity,
  buffIsMonitoring,
  gameHasUnsavedChanges,
  macroHasUnsavedChanges
}: InstallBlockingState): string | null {
  if (macroIsRunning) return '宏正在执行，请先停止执行再安装更新。'
  if (macroIsRecording) return '正在录制流程，请先停止录制再安装更新。'
  if (gameActivity === 'recordingCountdown' || gameActivity === 'recording') {
    return '游戏操作正在录制，请先停止录制再安装更新。'
  }
  if (gameActivity === 'playbackCountdown' || gameActivity === 'playing') {
    return '游戏操作正在回放，请先停止回放再安装更新。'
  }
  if (buffIsMonitoring || buffActivity === 'testing') {
    return 'Buff 助手正在捕获游戏画面，请先停止监控或测试再安装更新。'
  }
  if (gameHasUnsavedChanges) {
    return '游戏录制有未保存的配置，请先保存或撤销后再安装更新。'
  }
  if (macroHasUnsavedChanges) return '当前有未保存的编辑，请先保存或撤销后再安装更新。'
  return null
}
