import {
  BellRing,
  Eye,
  ImagePlus,
  MonitorPlay,
  Pause,
  Play,
  RefreshCw,
  Save,
  ScanSearch,
  Settings2,
  Square,
  Trash2,
  Volume2
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

import { Button } from '../../components/ui/button'
import type { BuffAssistantController } from '../../hooks/useBuffAssistantController'
import type { BuffAssistantSettings, NormalizedRect } from '../../lib/macro-api'
import { MaskEditor, type MaskEditorHandle } from './MaskEditor'
import { RegionSelector } from './RegionSelector'

import './BuffAssistantPage.css'

type BuffAssistantPageProps = {
  controller: BuffAssistantController
}

const defaultRegion: NormalizedRect = { x: 0.55, y: 0.02, width: 0.4, height: 0.16 }

export function BuffAssistantPage({ controller }: BuffAssistantPageProps) {
  const {
    state,
    windows,
    preview,
    samples,
    selectedFrame,
    metric,
    busy,
    error,
    refreshWindows,
    capturePreview,
    startSampleCapture,
    pauseSampleCapture,
    loadSamples,
    loadSampleFrame,
    saveTemplate,
    deleteTemplate,
    updateSettings,
    startMonitor,
    stopMonitor,
    startTest,
    stopTest,
    setOverlayEditing
  } = controller
  const [selectedWindowId, setSelectedWindowId] = useState('')
  const [searchRegion, setSearchRegion] = useState<NormalizedRect | null>(null)
  const [selectedSampleId, setSelectedSampleId] = useState<number | null>(null)
  const [templateCrop, setTemplateCrop] = useState<NormalizedRect | null>(null)
  const [settings, setSettings] = useState<BuffAssistantSettings>(state.config.settings)
  const [overlayEditing, setOverlayEditingState] = useState(false)
  const maskRef = useRef<MaskEditorHandle>(null)

  useEffect(() => {
    setSettings(state.config.settings)
  }, [state.config.settings])

  useEffect(() => {
    void refreshWindows().catch(() => undefined)
  }, [refreshWindows])

  useEffect(() => {
    if (selectedWindowId || windows.length === 0) return
    const configured = state.config.target
      ? windows.find(
          (candidate) =>
            candidate.processName.toLowerCase() ===
              state.config.target?.processName.toLowerCase() &&
            candidate.windowTitle === state.config.target.windowTitle
        )
      : undefined
    setSelectedWindowId((configured ?? windows[0]).id)
  }, [selectedWindowId, state.config.target, windows])

  const status = describeStatus(state.activity, state.isMonitoring)
  const hasTemplate = Boolean(
    state.config.template && state.config.target && state.config.searchRegion
  )
  const selectedSample = samples.find((sample) => sample.id === selectedSampleId)

  async function handlePreview(): Promise<void> {
    if (!selectedWindowId) return
    const result = await capturePreview(selectedWindowId)
    setSearchRegion(state.config.searchRegion ?? defaultRegion)
    setSelectedSampleId(null)
    setTemplateCrop(null)
    if (result.width < 1) setSearchRegion(null)
  }

  async function handleStartCapture(): Promise<void> {
    if (!selectedWindowId || !searchRegion) return
    await startSampleCapture(selectedWindowId, searchRegion)
  }

  async function handleLoadSamples(): Promise<void> {
    if (state.activity === 'capturingSamples') await pauseSampleCapture()
    const frames = await loadSamples()
    const latest = frames[frames.length - 1]
    if (!latest) return
    setSelectedSampleId(latest.id)
    await loadSampleFrame(latest.id)
    setTemplateCrop({ x: 0.05, y: 0.05, width: 0.2, height: 0.8 })
  }

  async function handleSelectSample(id: number): Promise<void> {
    setSelectedSampleId(id)
    await loadSampleFrame(id)
    setTemplateCrop(null)
  }

  async function handleSaveTemplate(): Promise<void> {
    if (!selectedSampleId || !templateCrop) return
    await saveTemplate(selectedSampleId, templateCrop, maskRef.current?.getMaskDataUrl())
  }

  async function handleOverlayEdit(): Promise<void> {
    const next = !overlayEditing
    await setOverlayEditing(next)
    setOverlayEditingState(next)
  }

  return (
    <div className="buff-assistant-page">
      <section className="buff-assistant-hero">
        <div>
          <span className="buff-assistant-eyebrow">金周天 · 屏幕识别</span>
          <h2>自动监听真实触发，脱战后自动丢弃旧时间轴</h2>
          <p>第一次识别成功后建立固定 20 秒轴；提前 3 秒和 1 秒预警，到点未出现则重新等待。</p>
        </div>
        <div className="buff-assistant-status" data-status={state.activity}>
          <span className="buff-assistant-status__dot" />
          <div>
            <small>当前状态</small>
            <strong>{status}</strong>
          </div>
        </div>
      </section>

      {error || state.lastError ? (
        <div className="buff-assistant-error" role="alert">
          {error ?? state.lastError}
        </div>
      ) : null}

      <section className="buff-assistant-grid">
        <article className="buff-card buff-card--runtime">
          <header>
            <div>
              <BellRing aria-hidden="true" />
              <div>
                <h3>日常监控</h3>
                <p>开始后可关闭主窗口到托盘，识别和悬浮提示会继续运行。</p>
              </div>
            </div>
          </header>
          <div className="buff-runtime-summary">
            <div>
              <span>模板</span>
              <strong>{state.config.template ? '金周天已配置' : '尚未采集'}</strong>
            </div>
            <div>
              <span>识别阈值</span>
              <strong>{Math.round(state.config.settings.threshold * 100)}%</strong>
            </div>
            <div>
              <span>固定周期</span>
              <strong>{(state.config.settings.cycleMs / 1000).toFixed(1)} 秒</strong>
            </div>
          </div>
          <div className="buff-card__actions">
            {state.isMonitoring ? (
              <Button disabled={busy} variant="destructive" onClick={() => void stopMonitor()}>
                <Square aria-hidden="true" />
                停止监控
              </Button>
            ) : (
              <Button disabled={busy || !hasTemplate} onClick={() => void startMonitor()}>
                <Play aria-hidden="true" />
                开始监控
              </Button>
            )}
            <Button disabled={busy} variant="outline" onClick={() => void handleOverlayEdit()}>
              <MonitorPlay aria-hidden="true" />
              {overlayEditing ? '保存悬浮位置' : '调整悬浮位置'}
            </Button>
          </div>
        </article>

        <article className="buff-card">
          <header>
            <div>
              <Settings2 aria-hidden="true" />
              <div>
                <h3>识别与提醒设置</h3>
                <p>更改识别参数后，正在运行的监控会重新等待首次触发。</p>
              </div>
            </div>
          </header>
          <div className="buff-settings-grid">
            <label>
              <span>周期（秒）</span>
              <input
                max={120}
                min={5}
                step={0.1}
                type="number"
                value={settings.cycleMs / 1000}
                onChange={(event) =>
                  setSettings((current) => ({
                    ...current,
                    cycleMs: Math.round(Number(event.target.value) * 1000)
                  }))
                }
              />
            </label>
            <label>
              <span>匹配阈值</span>
              <input
                max={0.99}
                min={0.5}
                step={0.01}
                type="number"
                value={settings.threshold}
                onChange={(event) =>
                  setSettings((current) => ({
                    ...current,
                    threshold: Number(event.target.value)
                  }))
                }
              />
            </label>
            <label>
              <span>确认帧数</span>
              <input
                max={12}
                min={1}
                type="number"
                value={settings.confirmFrames}
                onChange={(event) =>
                  setSettings((current) => ({
                    ...current,
                    confirmFrames: Number(event.target.value)
                  }))
                }
              />
            </label>
            <label>
              <span>消失帧数</span>
              <input
                max={30}
                min={1}
                type="number"
                value={settings.missingFrames}
                onChange={(event) =>
                  setSettings((current) => ({
                    ...current,
                    missingFrames: Number(event.target.value)
                  }))
                }
              />
            </label>
          </div>
          <div className="buff-sound-options">
            <ToggleRow
              checked={settings.sound.triggerEnabled}
              label="真实触发确认音"
              onChange={(checked) =>
                setSettings((current) => ({
                  ...current,
                  sound: { ...current.sound, triggerEnabled: checked }
                }))
              }
              onTest={() => void window.api.playBuffAssistantSound('triggered')}
            />
            <ToggleRow
              checked={settings.sound.prewarnThreeEnabled}
              label="提前 3 秒提示音"
              onChange={(checked) =>
                setSettings((current) => ({
                  ...current,
                  sound: { ...current.sound, prewarnThreeEnabled: checked }
                }))
              }
              onTest={() => void window.api.playBuffAssistantSound('prewarnThree')}
            />
            <ToggleRow
              checked={settings.sound.prewarnOneEnabled}
              label="提前 1 秒提示音"
              onChange={(checked) =>
                setSettings((current) => ({
                  ...current,
                  sound: { ...current.sound, prewarnOneEnabled: checked }
                }))
              }
              onTest={() => void window.api.playBuffAssistantSound('prewarnOne')}
            />
            <label className="buff-volume-row">
              <Volume2 aria-hidden="true" />
              <span>提示音量</span>
              <input
                max={1}
                min={0}
                step={0.05}
                type="range"
                value={settings.sound.volume}
                onChange={(event) =>
                  setSettings((current) => ({
                    ...current,
                    sound: { ...current.sound, volume: Number(event.target.value) }
                  }))
                }
              />
              <strong>{Math.round(settings.sound.volume * 100)}%</strong>
            </label>
            <label className="buff-check-row">
              <input
                checked={settings.overlay.showWaitingDot}
                type="checkbox"
                onChange={(event) =>
                  setSettings((current) => ({
                    ...current,
                    overlay: { ...current.overlay, showWaitingDot: event.target.checked }
                  }))
                }
              />
              等待阶段显示状态点
            </label>
          </div>
          <div className="buff-card__actions">
            <Button disabled={busy} onClick={() => void updateSettings(settings)}>
              <Save aria-hidden="true" />
              保存设置
            </Button>
          </div>
        </article>
      </section>

      <section className="buff-card buff-template-wizard">
        <header>
          <div>
            <ImagePlus aria-hidden="true" />
            <div>
              <h3>采集金周天图标模板</h3>
              <p>选择窗口、框选 Buff 栏、正常战斗，再从最近画面中裁出金周天图标。</p>
            </div>
          </div>
          <Button disabled={busy} size="sm" variant="outline" onClick={() => void refreshWindows()}>
            <RefreshCw aria-hidden="true" />
            刷新窗口
          </Button>
        </header>

        <div className="buff-window-row">
          <select
            aria-label="目标游戏窗口"
            value={selectedWindowId}
            onChange={(event) => setSelectedWindowId(event.target.value)}
          >
            {windows.length === 0 ? <option value="">没有可捕获窗口</option> : null}
            {windows.map((candidate) => (
              <option key={candidate.id} value={candidate.id}>
                {candidate.processName} · {candidate.windowTitle} · {candidate.width}×
                {candidate.height}
              </option>
            ))}
          </select>
          <Button
            disabled={busy || !selectedWindowId}
            variant="outline"
            onClick={() => void handlePreview()}
          >
            <Eye aria-hidden="true" />
            捕获预览
          </Button>
        </div>

        {preview ? (
          <div className="buff-wizard-step">
            <div className="buff-wizard-step__title">
              <span>1</span>
              <div>
                <strong>框选 Buff 栏搜索区域</strong>
                <p>区域越小识别越快，但要覆盖金周天可能出现的位置。</p>
              </div>
            </div>
            <RegionSelector
              imageUrl={preview.dataUrl}
              label="Buff 搜索区域"
              value={searchRegion}
              onChange={setSearchRegion}
            />
            <div className="buff-card__actions">
              {state.activity === 'capturingSamples' ? (
                <Button disabled={busy} variant="outline" onClick={() => void pauseSampleCapture()}>
                  <Pause aria-hidden="true" />
                  暂停采集
                </Button>
              ) : (
                <Button disabled={busy || !searchRegion} onClick={() => void handleStartCapture()}>
                  <ScanSearch aria-hidden="true" />
                  开始采集并隐藏窗口
                </Button>
              )}
              <span className="buff-action-hint">
                正常游戏后双击托盘图标返回，画面会自动暂停并保留最近 120 秒。
              </span>
            </div>
          </div>
        ) : null}

        {state.sampleCount > 0 ? (
          <div className="buff-wizard-step">
            <div className="buff-wizard-step__title">
              <span>2</span>
              <div>
                <strong>选择包含金周天的画面</strong>
                <p>当前缓存 {state.sampleCount} 帧，优先选择图标清晰且闪光较少的一帧。</p>
              </div>
            </div>
            <Button disabled={busy} variant="outline" onClick={() => void handleLoadSamples()}>
              载入最近画面
            </Button>
            {samples.length > 0 ? (
              <div className="buff-sample-strip">
                {samples.map((sample) => (
                  <button
                    className="buff-sample-thumb"
                    data-selected={sample.id === selectedSampleId}
                    key={sample.id}
                    type="button"
                    onClick={() => void handleSelectSample(sample.id)}
                  >
                    <img alt={`采集帧 ${sample.id}`} src={sample.thumbnailDataUrl} />
                    <span>{new Date(sample.capturedAtUnixMs).toLocaleTimeString()}</span>
                  </button>
                ))}
              </div>
            ) : null}
          </div>
        ) : null}

        {selectedFrame && selectedSample ? (
          <div className="buff-wizard-step">
            <div className="buff-wizard-step__title">
              <span>3</span>
              <div>
                <strong>裁剪金周天图标</strong>
                <p>只框选图标主体，尽量不要包含相邻 Buff。</p>
              </div>
            </div>
            <RegionSelector
              imageUrl={selectedFrame}
              label="金周天图标"
              value={templateCrop}
              onChange={setTemplateCrop}
            />
            {templateCrop ? (
              <>
                <div className="buff-wizard-step__title buff-wizard-step__title--sub">
                  <span>4</span>
                  <div>
                    <strong>涂抹忽略区域</strong>
                    <p>在倒计时数字、层数或动态闪光上涂抹；不需要时可直接保存。</p>
                  </div>
                </div>
                <MaskEditor crop={templateCrop} imageUrl={selectedFrame} ref={maskRef} />
                <div className="buff-card__actions">
                  <Button disabled={busy} onClick={() => void handleSaveTemplate()}>
                    <Save aria-hidden="true" />
                    保存金周天模板
                  </Button>
                </div>
              </>
            ) : null}
          </div>
        ) : null}

        {hasTemplate ? (
          <div className="buff-template-test">
            <div>
              <strong>实时识别测试</strong>
              <span>
                置信度 {Math.round(metric.confidence * 100)}% ·{' '}
                {metric.present ? '已确认图标' : '未确认'}
              </span>
              <div className="buff-confidence-track">
                <span style={{ width: `${Math.min(100, metric.confidence * 100)}%` }} />
              </div>
            </div>
            <div className="buff-card__actions">
              {state.activity === 'testing' ? (
                <Button disabled={busy} variant="outline" onClick={() => void stopTest()}>
                  停止测试
                </Button>
              ) : (
                <Button
                  disabled={busy || !selectedWindowId}
                  variant="outline"
                  onClick={() => void startTest(selectedWindowId)}
                >
                  开始测试
                </Button>
              )}
              <Button
                disabled={busy}
                variant="destructive"
                onClick={() => {
                  if (window.confirm('确定删除当前金周天模板吗？')) void deleteTemplate()
                }}
              >
                <Trash2 aria-hidden="true" />
                删除模板
              </Button>
            </div>
          </div>
        ) : null}
      </section>
    </div>
  )
}

type ToggleRowProps = {
  checked: boolean
  label: string
  onChange: (checked: boolean) => void
  onTest: () => void
}

function ToggleRow({ checked, label, onChange, onTest }: ToggleRowProps) {
  return (
    <div className="buff-toggle-row">
      <label>
        <input
          checked={checked}
          type="checkbox"
          onChange={(event) => onChange(event.target.checked)}
        />
        {label}
      </label>
      <button aria-label={`试听${label}`} type="button" onClick={onTest}>
        试听
      </button>
    </div>
  )
}

function describeStatus(activity: string, monitoring: boolean): string {
  if (!monitoring && activity === 'stopped') return '未开始'
  const labels: Record<string, string> = {
    waiting: '等待金周天',
    tracking: '20 秒计时中',
    prewarning: '即将触发',
    capturingSamples: '正在采集画面',
    testing: '模板测试中',
    targetUnavailable: '等待游戏窗口',
    error: '运行异常',
    stopped: '已停止'
  }
  return labels[activity] ?? '未知状态'
}
