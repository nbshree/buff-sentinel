import {
  Activity,
  CircleAlert,
  Clock3,
  Gamepad2,
  Keyboard,
  ListVideo,
  LoaderCircle,
  Mouse,
  Pencil,
  Play,
  Radio,
  Save,
  Square,
  Target,
  Trash2
} from 'lucide-react'

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger
} from '@/components/ui/alert-dialog'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/components/ui/select'
import { formatGameHotkey, type GameRecorderController } from '@/hooks/useGameRecorderController'
import type { GameRecorderActivity, GameRecorderHotkeys } from '@/lib/macro-api'

import './GameRecorderPage.css'

const hotkeyFields: Array<[keyof GameRecorderHotkeys, string]> = [
  ['recordStart', '开始录制'],
  ['stop', '停止录制/停止回放'],
  ['playbackStart', '开始回放']
]

const dateFormatter = new Intl.DateTimeFormat('zh-CN', {
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit'
})

type GameRecorderPageProps = {
  controller: GameRecorderController
}

function formatDuration(durationMs: number): string {
  const totalSeconds = Math.max(0, Math.round(durationMs / 1000))
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  return hours > 0
    ? `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
    : `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
}

function activityDescription(
  activity: GameRecorderActivity,
  countdownRemaining: number,
  blockedByMacro: boolean
): string {
  if (blockedByMacro && activity === 'idle') {
    return '宏流程正在录制或执行，请先停止宏任务再编辑或启动游戏录制'
  }
  switch (activity) {
    case 'recordingCountdown':
      return `${countdownRemaining} 秒后开始采集当前前台游戏输入`
    case 'recording':
      return '正在采集相对鼠标与键盘输入，按停止热键结束并保存'
    case 'playbackCountdown':
      return `${countdownRemaining} 秒后向当前前台游戏回放输入`
    case 'playing':
      return '正在按录制时间轴回放，保持游戏输入模式与灵敏度不变'
    default:
      return '录制和回放会在 3 秒倒计时后开始，应用不会抢回游戏焦点'
  }
}

function pendingLabel(controller: GameRecorderController, action: string, label: string): string {
  return controller.pendingAction === action ? '处理中…' : label
}

export function GameRecorderPage({ controller }: GameRecorderPageProps): React.JSX.Element {
  const {
    actionError,
    blockedByMacro,
    captureHotkey,
    capturingHotkey,
    deleteSelected,
    dismissTargetMismatch,
    draftHotkeys,
    draftPlayback,
    hasHotkeyChanges,
    hasNameChanges,
    hasPlaybackChanges,
    isBusy,
    isIdle,
    nameInput,
    pendingAction,
    progressLabel,
    renameSelected,
    saveHotkeys,
    savePlayback,
    selectRecording,
    selectedRecording,
    setNameInput,
    setPlaybackLoopMode,
    setPlaybackSpeed,
    startHotkeyCapture,
    startPlayback,
    startRecording,
    state,
    status,
    stopActivity,
    stopHotkeyCapture,
    targetMismatchPromptOpen,
    updatePlaybackNumber
  } = controller
  const error = actionError ?? state.lastError

  return (
    <div className="game-recorder-page">
      <aside className="game-recorder-sidebar" aria-label="游戏录制控制与录制库">
        <section className="ui-panel game-recorder-control" aria-labelledby="game-control-title">
          <div className="ui-panel__heading">
            <h2 id="game-control-title">
              <Gamepad2 aria-hidden="true" size={17} />
              录制控制
            </h2>
            <Badge className="game-recorder-status" data-tone={status.tone} variant="ghost">
              {status.label}
            </Badge>
          </div>

          <div className="game-recorder-activity" aria-live="polite">
            <span className="game-recorder-activity__icon" data-active={String(isBusy)}>
              {isBusy ? <Activity aria-hidden="true" /> : <Radio aria-hidden="true" />}
            </span>
            <div>
              <strong>{status.label}</strong>
              <p>{activityDescription(state.activity, state.countdownRemaining, blockedByMacro)}</p>
            </div>
          </div>

          <div className="game-recorder-actions">
            <Button
              disabled={!isIdle || Boolean(pendingAction)}
              type="button"
              onClick={() => void startRecording()}
            >
              {pendingAction === 'record' ? (
                <LoaderCircle aria-hidden="true" className="game-recorder-spinner" />
              ) : (
                <Radio aria-hidden="true" />
              )}
              {pendingLabel(controller, 'record', '开始录制')}
            </Button>
            <Button
              disabled={!isIdle || !selectedRecording || Boolean(pendingAction)}
              type="button"
              variant="outline"
              onClick={() => void startPlayback()}
            >
              <Play aria-hidden="true" />
              {pendingLabel(controller, 'playback', '回放选中')}
            </Button>
            <Button
              className="game-recorder-actions__stop"
              disabled={state.activity === 'idle' || Boolean(pendingAction)}
              type="button"
              variant="destructive"
              onClick={() => void stopActivity()}
            >
              <Square aria-hidden="true" />
              {pendingLabel(controller, 'stop', '停止当前任务')}
            </Button>
          </div>

          <p className="game-recorder-control__hint">
            单次最多 10 分钟或 50,000 个事件；紧急停止始终为 Ctrl+Alt+Esc。
          </p>
        </section>

        <section
          className="ui-panel game-recording-library"
          aria-labelledby="recording-library-title"
        >
          <div className="ui-panel__heading">
            <h2 id="recording-library-title">
              <ListVideo aria-hidden="true" size={17} />
              录制库
            </h2>
            <span className="ui-panel__count">{state.recordings.length} 条</span>
          </div>

          {state.recordings.length === 0 ? (
            <div className="game-recording-empty">
              <Mouse aria-hidden="true" size={25} />
              <strong>还没有游戏录制</strong>
              <span>切回游戏后使用开始录制热键，结束时会自动保存到这里。</span>
            </div>
          ) : (
            <div className="game-recording-list">
              {state.recordings.map((recording) => {
                const selected = recording.id === state.activeRecordingId
                return (
                  <button
                    aria-pressed={selected}
                    className="game-recording-item"
                    data-selected={String(selected)}
                    disabled={isBusy || Boolean(pendingAction)}
                    key={recording.id}
                    type="button"
                    onClick={() => void selectRecording(recording.id)}
                  >
                    <span className="game-recording-item__topline">
                      <strong>{recording.name}</strong>
                      <span>{formatDuration(recording.durationMs)}</span>
                    </span>
                    <span className="game-recording-item__target">
                      {recording.target.processName || '未知程序'}
                    </span>
                    <span className="game-recording-item__meta">
                      {recording.eventCount.toLocaleString('zh-CN')} 个事件 ·{' '}
                      {dateFormatter.format(recording.updatedAt)}
                    </span>
                  </button>
                )
              })}
            </div>
          )}
        </section>
      </aside>

      <section className="game-recorder-main" aria-label="游戏录制详情与配置">
        {error ? (
          <Alert className="game-recorder-error" variant="destructive">
            <CircleAlert aria-hidden="true" />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        {selectedRecording ? (
          <>
            <section
              className="ui-panel game-recording-summary"
              aria-labelledby="recording-detail-title"
            >
              <div className="game-recording-summary__heading">
                <div>
                  <span className="game-recording-summary__eyebrow">当前录制</span>
                  <h2 id="recording-detail-title">{selectedRecording.name}</h2>
                </div>
                <Badge variant="outline">{progressLabel}</Badge>
              </div>

              <div className="game-recording-target">
                <Target aria-hidden="true" />
                <div>
                  <span>目标游戏</span>
                  <strong>{selectedRecording.target.processName || '未知程序'}</strong>
                  <p title={selectedRecording.target.windowTitle}>
                    {selectedRecording.target.windowTitle || '未记录窗口标题'}
                  </p>
                </div>
              </div>

              <dl className="game-recording-stats">
                <div>
                  <dt>
                    <Clock3 aria-hidden="true" />
                    时长
                  </dt>
                  <dd>{formatDuration(selectedRecording.durationMs)}</dd>
                </div>
                <div>
                  <dt>
                    <Keyboard aria-hidden="true" />
                    键盘事件
                  </dt>
                  <dd>{selectedRecording.keyboardEventCount.toLocaleString('zh-CN')}</dd>
                </div>
                <div>
                  <dt>
                    <Mouse aria-hidden="true" />
                    鼠标事件
                  </dt>
                  <dd>{selectedRecording.mouseEventCount.toLocaleString('zh-CN')}</dd>
                </div>
                <div>
                  <dt>
                    <Activity aria-hidden="true" />
                    总事件
                  </dt>
                  <dd>{selectedRecording.eventCount.toLocaleString('zh-CN')}</dd>
                </div>
              </dl>

              <form
                className="game-recording-rename"
                onSubmit={(event) => {
                  event.preventDefault()
                  void renameSelected()
                }}
              >
                <div className="ui-field">
                  <Label htmlFor="game-recording-name">录制名称</Label>
                  <Input
                    id="game-recording-name"
                    disabled={isBusy}
                    maxLength={64}
                    value={nameInput}
                    onChange={(event) => setNameInput(event.target.value)}
                  />
                </div>
                <Button
                  disabled={
                    isBusy || !nameInput.trim() || !hasNameChanges || Boolean(pendingAction)
                  }
                  type="submit"
                  variant="outline"
                >
                  <Pencil aria-hidden="true" />
                  重命名
                </Button>
                <AlertDialog>
                  <AlertDialogTrigger asChild>
                    <Button
                      aria-label={`删除录制 ${selectedRecording.name}`}
                      disabled={isBusy || Boolean(pendingAction)}
                      type="button"
                      variant="destructive"
                    >
                      <Trash2 aria-hidden="true" />
                      删除
                    </Button>
                  </AlertDialogTrigger>
                  <AlertDialogContent size="sm">
                    <AlertDialogHeader>
                      <AlertDialogTitle>删除这条游戏录制？</AlertDialogTitle>
                      <AlertDialogDescription>
                        “{selectedRecording.name}”及其完整输入时间轴会从本机永久删除。
                      </AlertDialogDescription>
                    </AlertDialogHeader>
                    <AlertDialogFooter>
                      <AlertDialogCancel>取消</AlertDialogCancel>
                      <AlertDialogAction
                        variant="destructive"
                        onClick={() => void deleteSelected()}
                      >
                        删除录制
                      </AlertDialogAction>
                    </AlertDialogFooter>
                  </AlertDialogContent>
                </AlertDialog>
              </form>
            </section>
          </>
        ) : null}

        <div className="game-recorder-config-grid">
          {selectedRecording ? (
            <section className="ui-panel game-recorder-config" aria-labelledby="playback-title">
              <div className="ui-panel__heading">
                <h2 id="playback-title">
                  <Play aria-hidden="true" size={17} />
                  回放配置
                </h2>
              </div>

              <div className="game-recorder-settings-grid">
                <div className="ui-field">
                  <Label htmlFor="game-playback-speed">回放速度</Label>
                  <Select
                    disabled={isBusy}
                    value={String(draftPlayback.speed)}
                    onValueChange={setPlaybackSpeed}
                  >
                    <SelectTrigger id="game-playback-speed">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="0.5">0.5×</SelectItem>
                      <SelectItem value="1">1×</SelectItem>
                      <SelectItem value="1.5">1.5×</SelectItem>
                      <SelectItem value="2">2×</SelectItem>
                    </SelectContent>
                  </Select>
                </div>

                <div className="ui-field">
                  <Label htmlFor="game-loop-interval">轮间隔 s</Label>
                  <Input
                    id="game-loop-interval"
                    disabled={isBusy}
                    min="0"
                    step="0.1"
                    type="number"
                    value={draftPlayback.loopIntervalSeconds}
                    onChange={(event) =>
                      updatePlaybackNumber('loopIntervalSeconds', event.target.value)
                    }
                  />
                </div>

                <div className="ui-field ui-field--wide">
                  <Label id="game-loop-mode-label">循环模式</Label>
                  <RadioGroup
                    aria-labelledby="game-loop-mode-label"
                    className="game-recorder-loop-mode"
                    disabled={isBusy}
                    value={draftPlayback.loopMode}
                    onValueChange={setPlaybackLoopMode}
                  >
                    <div className="relative min-w-0">
                      <RadioGroupItem
                        className="peer sr-only"
                        id="game-loop-mode-count"
                        value="count"
                      />
                      <Label
                        className="peer-data-[state=checked]:text-ui-primary-foreground"
                        htmlFor="game-loop-mode-count"
                      >
                        指定次数
                      </Label>
                    </div>
                    <div className="relative min-w-0">
                      <RadioGroupItem
                        className="peer sr-only"
                        id="game-loop-mode-infinite"
                        value="infinite"
                      />
                      <Label
                        className="peer-data-[state=checked]:text-ui-primary-foreground"
                        htmlFor="game-loop-mode-infinite"
                      >
                        无限循环
                      </Label>
                    </div>
                  </RadioGroup>
                </div>

                <div className="ui-field ui-field--wide">
                  <Label htmlFor="game-loop-count">循环次数</Label>
                  <Input
                    id="game-loop-count"
                    disabled={isBusy || draftPlayback.loopMode === 'infinite'}
                    min="1"
                    step="1"
                    type="number"
                    value={draftPlayback.loopCount}
                    onChange={(event) => updatePlaybackNumber('loopCount', event.target.value)}
                  />
                </div>
              </div>

              <Button
                className="game-recorder-save"
                disabled={isBusy || !hasPlaybackChanges || Boolean(pendingAction)}
                type="button"
                onClick={() => void savePlayback()}
              >
                <Save aria-hidden="true" />
                保存回放配置
              </Button>
            </section>
          ) : (
            <section className="ui-panel game-recorder-welcome">
              <Gamepad2 aria-hidden="true" size={42} />
              <h2>录制游戏中的镜头与按键操作</h2>
              <p>
                点击“开始录制”或在游戏中按 {formatGameHotkey(state.hotkeys.recordStart)}。
                倒计时结束后，系统会记录相对鼠标移动、左右中键、滚轮和键盘扫描码，
                并在停止时自动保存。
              </p>
              <div className="game-recorder-welcome__notes">
                <span>仅支持相对游戏输入，不保存桌面坐标</span>
                <span>反作弊、独占输入或管理员窗口可能拒绝模拟输入</span>
              </div>
            </section>
          )}

          <section className="ui-panel game-recorder-config" aria-labelledby="game-hotkeys-title">
            <div className="ui-panel__heading">
              <h2 id="game-hotkeys-title">
                <Keyboard aria-hidden="true" size={17} />
                全局热键
              </h2>
            </div>

            <div className="game-recorder-hotkeys">
              {hotkeyFields.map(([key, label]) => {
                const inputId = `game-hotkey-${key}`
                const isCapturing = capturingHotkey === key
                return (
                  <div className="ui-field" key={key}>
                    <Label htmlFor={inputId}>{label}</Label>
                    <Input
                      readOnly
                      aria-label={`${label}快捷键`}
                      data-capturing={String(isCapturing)}
                      disabled={isBusy}
                      id={inputId}
                      value={isCapturing ? '请按组合键...' : formatGameHotkey(draftHotkeys[key])}
                      onBlur={stopHotkeyCapture}
                      onFocus={() => startHotkeyCapture(key)}
                      onKeyDown={(event) => captureHotkey(event, key)}
                    />
                  </div>
                )
              })}
            </div>

            {state.hotkeyErrors.length > 0 ? (
              <Alert className="game-recorder-hotkey-errors" variant="destructive">
                <CircleAlert aria-hidden="true" />
                <AlertDescription>
                  {state.hotkeyErrors.map((message) => (
                    <p key={message}>{message}</p>
                  ))}
                </AlertDescription>
              </Alert>
            ) : null}

            <Button
              className="game-recorder-save"
              disabled={isBusy || !hasHotkeyChanges || Boolean(pendingAction)}
              type="button"
              onClick={() => void saveHotkeys()}
            >
              <Save aria-hidden="true" />
              保存全局热键
            </Button>
          </section>
        </div>
      </section>

      <AlertDialog
        open={targetMismatchPromptOpen}
        onOpenChange={(open) => {
          if (!open) dismissTargetMismatch()
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>当前前台程序与录制目标不一致</AlertDialogTitle>
            <AlertDialogDescription>
              这条录制来自“{selectedRecording?.target.processName || '未知程序'}”
              {selectedRecording?.target.windowTitle
                ? `（${selectedRecording.target.windowTitle}）`
                : ''}
              。继续后会重新倒计时 3 秒，并把输入发送到届时的前台程序。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={dismissTargetMismatch}>取消回放</AlertDialogCancel>
            <AlertDialogAction onClick={() => void startPlayback(true)}>仍然回放</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
