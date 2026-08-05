import {
  BellRing,
  ExternalLink,
  Eye,
  ImagePlus,
  MonitorPlay,
  Play,
  RefreshCw,
  Save,
  ScrollText,
  Settings2,
  Square,
  Trash2,
  Upload,
  Volume2
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

import { Button } from '../../components/ui/button'
import type { BuffAssistantController } from '../../hooks/useBuffAssistantController'
import type {
  BuffAssistantSettings,
  BuffSoundCue,
  BuffSoundSource,
  BuffSoundTemplateSummary,
  NormalizedRect
} from '../../lib/macro-api'
import {
  createMaskHistory,
  MaskEditor,
  type MaskEditorHandle,
  type MaskHistory
} from './MaskEditor'
import { MaskEditorDialog } from './MaskEditorDialog'
import { RegionEditorDialog } from './RegionEditorDialog'
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
    metric,
    logs,
    busy,
    error,
    refreshWindows,
    capturePreview,
    saveTemplate,
    deleteTemplate,
    updateSettings,
    startMonitor,
    stopMonitor,
    startTest,
    stopTest,
    clearLogs,
    setOverlayEditing
  } = controller
  const [selectedWindowId, setSelectedWindowId] = useState('')
  const [searchRegion, setSearchRegion] = useState<NormalizedRect | null>(null)
  const [templateSource, setTemplateSource] = useState<string | null>(null)
  const [templateCrop, setTemplateCrop] = useState<NormalizedRect | null>(null)
  const [maskHistory, setMaskHistory] = useState<MaskHistory>(() => createMaskHistory())
  const [searchRegionEditorOpen, setSearchRegionEditorOpen] = useState(false)
  const [templateCropEditorOpen, setTemplateCropEditorOpen] = useState(false)
  const [maskEditorOpen, setMaskEditorOpen] = useState(false)
  const [settings, setSettings] = useState<BuffAssistantSettings>(state.config.settings)
  const [overlayEditing, setOverlayEditingState] = useState(false)
  const [soundTemplates, setSoundTemplates] = useState<BuffSoundTemplateSummary[]>([])
  const [soundError, setSoundError] = useState<string | null>(null)
  const [uploadingCue, setUploadingCue] = useState<BuffSoundCue | null>(null)
  const maskRef = useRef<MaskEditorHandle>(null)

  useEffect(() => {
    setSettings(state.config.settings)
  }, [state.config.settings])

  useEffect(() => {
    void refreshWindows().catch(() => undefined)
  }, [refreshWindows])

  useEffect(() => {
    let disposed = false
    void window.api
      .listBuffSoundTemplates()
      .then((templates) => {
        if (!disposed) setSoundTemplates(templates)
      })
      .catch((reason: unknown) => {
        if (!disposed) setSoundError(toMessage(reason))
      })
    return () => {
      disposed = true
    }
  }, [])

  async function importSound(cue: BuffSoundCue, field: SoundSourceField): Promise<void> {
    setUploadingCue(cue)
    setSoundError(null)
    try {
      const asset = await window.api.importBuffAssistantSound(cue)
      if (!asset) return
      setSettings((current) => ({
        ...current,
        sound: {
          ...current.sound,
          [field]: { type: 'custom', assetId: asset.assetId, fileName: asset.fileName }
        }
      }))
    } catch (reason) {
      setSoundError(toMessage(reason))
    } finally {
      setUploadingCue(null)
    }
  }

  async function previewSound(cue: BuffSoundCue, source: BuffSoundSource): Promise<void> {
    setSoundError(null)
    try {
      await window.api.playBuffAssistantSound(cue, source, settings.sound.volume)
    } catch (reason) {
      setSoundError(toMessage(reason))
    }
  }

  async function openTtsOnline(): Promise<void> {
    setSoundError(null)
    try {
      await window.api.openTtsOnline()
    } catch (reason) {
      setSoundError(toMessage(reason))
    }
  }

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

  useEffect(() => {
    let disposed = false
    setTemplateSource(null)
    if (!preview || !searchRegion) return

    void cropImageDataUrl(preview.dataUrl, searchRegion)
      .then((dataUrl) => {
        if (!disposed) setTemplateSource(dataUrl)
      })
      .catch((reason: unknown) => {
        if (!disposed) console.error('裁剪 Buff 搜索区域失败', reason)
      })

    return () => {
      disposed = true
    }
  }, [preview, searchRegion])

  const status = describeStatus(state.activity, state.isMonitoring)
  const hasTemplate = Boolean(
    state.config.template && state.config.target && state.config.searchRegion
  )
  async function handlePreview(): Promise<void> {
    if (!selectedWindowId) return
    const result = await capturePreview(selectedWindowId)
    setSearchRegion(state.config.searchRegion ?? defaultRegion)
    setTemplateCrop(null)
    setMaskHistory(createMaskHistory())
    if (result.width < 1) setSearchRegion(null)
  }

  function handleSearchRegionChange(region: NormalizedRect): void {
    setSearchRegion(region)
    setTemplateCrop(null)
    setMaskHistory(createMaskHistory())
  }

  function handleTemplateCropChange(crop: NormalizedRect): void {
    setTemplateCrop(crop)
    setMaskHistory(createMaskHistory())
  }

  async function handleSaveTemplate(): Promise<void> {
    if (!searchRegion || !templateCrop) return
    await saveTemplate(searchRegion, templateCrop, maskRef.current?.getMaskDataUrl())
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
          <p>第一次识别成功后立即显示固定 20 秒倒计时；提前 3、2、1 秒预警。</p>
        </div>
        <div
          aria-label={`当前状态：${status}`}
          className="buff-assistant-status"
          data-status={state.activity}
        >
          <span className="buff-assistant-status__dot" />
          <strong>{status}</strong>
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
              <strong>{state.config.template ? '金周天已配置' : '尚未配置'}</strong>
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
                <p>更改识别参数后，正在运行的监控会重新等待脱战后的下一次触发。</p>
              </div>
            </div>
          </header>
          <div className="buff-settings-grid">
            <label>
              <span>浮窗配色</span>
              <select
                aria-label="浮窗配色"
                value={settings.overlay.colorScheme}
                onChange={(event) =>
                  setSettings((current) => ({
                    ...current,
                    overlay: {
                      ...current.overlay,
                      colorScheme: event.target
                        .value as BuffAssistantSettings['overlay']['colorScheme']
                    }
                  }))
                }
              >
                <option value="gold">金色（当前）</option>
                <option value="blackWhite">黑底白字</option>
              </select>
            </label>
            <label>
              <span>周期（秒）</span>
              <input
                max={120}
                min={5}
                step={0.01}
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
              <span>触发宽限期（毫秒）</span>
              <input
                max={2000}
                min={0}
                step={50}
                type="number"
                value={settings.deadlineGraceMs}
                onChange={(event) =>
                  setSettings((current) => ({
                    ...current,
                    deadlineGraceMs: Number(event.target.value)
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
            <label className="buff-check-row">
              <input
                checked={settings.overlay.showBorder}
                type="checkbox"
                onChange={(event) =>
                  setSettings((current) => ({
                    ...current,
                    overlay: { ...current.overlay, showBorder: event.target.checked }
                  }))
                }
              />
              显示浮窗边框
            </label>
            <SoundRow
              checked={settings.sound.triggerEnabled}
              cue="triggered"
              label="真实触发确认音"
              source={settings.sound.triggerSource}
              templates={soundTemplates}
              uploading={uploadingCue === 'triggered'}
              onChange={(checked) =>
                setSettings((current) => ({
                  ...current,
                  sound: { ...current.sound, triggerEnabled: checked }
                }))
              }
              onSourceChange={(source) =>
                setSettings((current) => ({
                  ...current,
                  sound: { ...current.sound, triggerSource: source }
                }))
              }
              onTest={() => void previewSound('triggered', settings.sound.triggerSource)}
              onUpload={() => void importSound('triggered', 'triggerSource')}
            />
            <SoundRow
              checked={settings.sound.prewarnThreeEnabled}
              cue="prewarnThree"
              label="倒计时 3 秒提示音"
              source={settings.sound.prewarnThreeSource}
              templates={soundTemplates}
              uploading={uploadingCue === 'prewarnThree'}
              onChange={(checked) =>
                setSettings((current) => ({
                  ...current,
                  sound: { ...current.sound, prewarnThreeEnabled: checked }
                }))
              }
              onSourceChange={(source) =>
                setSettings((current) => ({
                  ...current,
                  sound: { ...current.sound, prewarnThreeSource: source }
                }))
              }
              onTest={() => void previewSound('prewarnThree', settings.sound.prewarnThreeSource)}
              onUpload={() => void importSound('prewarnThree', 'prewarnThreeSource')}
            />
            <SoundRow
              checked={settings.sound.prewarnTwoEnabled}
              cue="prewarnTwo"
              label="倒计时 2 秒提示音"
              source={settings.sound.prewarnTwoSource}
              templates={soundTemplates}
              uploading={uploadingCue === 'prewarnTwo'}
              onChange={(checked) =>
                setSettings((current) => ({
                  ...current,
                  sound: { ...current.sound, prewarnTwoEnabled: checked }
                }))
              }
              onSourceChange={(source) =>
                setSettings((current) => ({
                  ...current,
                  sound: { ...current.sound, prewarnTwoSource: source }
                }))
              }
              onTest={() => void previewSound('prewarnTwo', settings.sound.prewarnTwoSource)}
              onUpload={() => void importSound('prewarnTwo', 'prewarnTwoSource')}
            />
            <SoundRow
              checked={settings.sound.prewarnOneEnabled}
              cue="prewarnOne"
              label="倒计时 1 秒提示音"
              source={settings.sound.prewarnOneSource}
              templates={soundTemplates}
              uploading={uploadingCue === 'prewarnOne'}
              onChange={(checked) =>
                setSettings((current) => ({
                  ...current,
                  sound: { ...current.sound, prewarnOneEnabled: checked }
                }))
              }
              onSourceChange={(source) =>
                setSettings((current) => ({
                  ...current,
                  sound: { ...current.sound, prewarnOneSource: source }
                }))
              }
              onTest={() => void previewSound('prewarnOne', settings.sound.prewarnOneSource)}
              onUpload={() => void importSound('prewarnOne', 'prewarnOneSource')}
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
            <div className="buff-sound-tip">
              <p>没有合适的提示音？可前往 TTS Online 将文本转换为语音，再下载 WAV 上传。</p>
              <button type="button" onClick={() => void openTtsOnline()}>
                <ExternalLink aria-hidden="true" />
                前往 TTS Online
              </button>
            </div>
            {soundError ? <p className="buff-sound-error">{soundError}</p> : null}
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
              <h3>配置金周天图标模板</h3>
              <p>捕获包含金周天的画面，框选 Buff 栏后直接裁出图标主体。</p>
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
              onChange={handleSearchRegionChange}
              onRequestExpand={() => setSearchRegionEditorOpen(true)}
            />
          </div>
        ) : null}

        {templateSource ? (
          <div className="buff-wizard-step">
            <div className="buff-wizard-step__title">
              <span>2</span>
              <div>
                <strong>裁剪金周天图标主体</strong>
                <p>下方仅显示刚才框选的 Buff 搜索区域，不要包含相邻 Buff。</p>
              </div>
            </div>
            <RegionSelector
              imageUrl={templateSource}
              label="金周天图标"
              value={templateCrop}
              onChange={handleTemplateCropChange}
              onRequestExpand={() => setTemplateCropEditorOpen(true)}
            />
            {templateCrop ? (
              <>
                <div className="buff-wizard-step__title buff-wizard-step__title--sub">
                  <span>3</span>
                  <div>
                    <strong>涂抹忽略区域</strong>
                    <p>在倒计时数字、层数或动态闪光上涂抹；不需要时可直接保存。</p>
                  </div>
                </div>
                <MaskEditor
                  crop={templateCrop}
                  imageUrl={templateSource}
                  ref={maskRef}
                  value={maskHistory}
                  onChange={setMaskHistory}
                  onRequestExpand={() => setMaskEditorOpen(true)}
                />
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

      {preview ? (
        <RegionEditorDialog
          description="框内拖动可整体移动，拖动四边或四角可精确调整搜索范围。"
          imageUrl={preview.dataUrl}
          label="Buff 搜索区域"
          open={searchRegionEditorOpen}
          title="精调 Buff 栏搜索区域"
          value={searchRegion}
          warning={templateCrop ? '应用新的搜索区域后，将清空图标裁剪和忽略区域。' : undefined}
          onApply={handleSearchRegionChange}
          onOpenChange={setSearchRegionEditorOpen}
        />
      ) : null}

      {templateSource ? (
        <RegionEditorDialog
          description="框内拖动可整体移动，拖动四边或四角可贴合金周天图标主体。"
          imageUrl={templateSource}
          label="金周天图标"
          open={templateCropEditorOpen}
          title="精调金周天图标主体"
          value={templateCrop}
          warning={
            maskHistory.present.length > 0
              ? '应用新的图标范围后，将清空已涂抹的忽略区域。'
              : undefined
          }
          onApply={handleTemplateCropChange}
          onOpenChange={setTemplateCropEditorOpen}
        />
      ) : null}

      {templateSource && templateCrop ? (
        <MaskEditorDialog
          crop={templateCrop}
          imageUrl={templateSource}
          open={maskEditorOpen}
          value={maskHistory}
          onApply={setMaskHistory}
          onOpenChange={setMaskEditorOpen}
        />
      ) : null}

      <section className="buff-execution-log" aria-labelledby="buff-execution-log-title">
        <header>
          <h3 id="buff-execution-log-title">
            <ScrollText aria-hidden="true" />
            执行日志
          </h3>
          <Button
            aria-label="清空执行日志"
            disabled={logs.length === 0}
            size="icon-compact"
            title="清空日志"
            type="button"
            variant="outline"
            onClick={clearLogs}
          >
            <Trash2 aria-hidden="true" />
          </Button>
        </header>
        <div className="buff-execution-log__body" aria-live="polite">
          {logs.length === 0 ? (
            <p className="buff-execution-log__empty">暂无日志。</p>
          ) : (
            logs.map((item, index) => <p key={`${index}-${item}`}>{item}</p>)
          )}
        </div>
      </section>
    </div>
  )
}

type SoundSourceField =
  'triggerSource' | 'prewarnThreeSource' | 'prewarnTwoSource' | 'prewarnOneSource'

type SoundRowProps = {
  checked: boolean
  cue: BuffSoundCue
  label: string
  source: BuffSoundSource
  templates: BuffSoundTemplateSummary[]
  uploading: boolean
  onChange: (checked: boolean) => void
  onSourceChange: (source: BuffSoundSource) => void
  onTest: () => void
  onUpload: () => void
}

function SoundRow({
  checked,
  cue,
  label,
  source,
  templates,
  uploading,
  onChange,
  onSourceChange,
  onTest,
  onUpload
}: SoundRowProps) {
  const value =
    source.type === 'template'
      ? `template:${source.templateId}`
      : source.type === 'custom'
        ? `custom:${source.assetId}`
        : 'sine'

  return (
    <div className="buff-sound-row" data-cue={cue}>
      <label>
        <input
          checked={checked}
          type="checkbox"
          onChange={(event) => onChange(event.target.checked)}
        />
        {label}
      </label>
      <select
        aria-label={`${label}来源`}
        value={value}
        onChange={(event) => {
          const next = event.target.value
          if (next === 'sine') {
            onSourceChange({ type: 'sine' })
          } else if (next.startsWith('template:')) {
            onSourceChange({ type: 'template', templateId: next.slice('template:'.length) })
          }
        }}
      >
        <option value="sine">正弦波</option>
        {templates.map((template) => (
          <option key={template.id} value={`template:${template.id}`}>
            {template.name}
          </option>
        ))}
        {source.type === 'custom' ? (
          <option value={`custom:${source.assetId}`}>自定义：{source.fileName}</option>
        ) : null}
      </select>
      <button
        aria-label={`上传${label} WAV`}
        className="buff-sound-row__upload"
        disabled={uploading}
        type="button"
        onClick={onUpload}
      >
        <Upload aria-hidden="true" />
        {uploading ? '选择中' : '上传'}
      </button>
      <button aria-label={`试听${label}`} type="button" onClick={onTest}>
        试听
      </button>
    </div>
  )
}

function cropImageDataUrl(imageUrl: string, region: NormalizedRect): Promise<string> {
  return new Promise((resolve, reject) => {
    const image = new Image()
    image.decoding = 'async'
    image.onload = () => {
      const startX = clamp(Math.floor(image.naturalWidth * region.x), 0, image.naturalWidth - 1)
      const startY = clamp(Math.floor(image.naturalHeight * region.y), 0, image.naturalHeight - 1)
      const endX = clamp(
        Math.ceil(image.naturalWidth * (region.x + region.width)),
        startX + 1,
        image.naturalWidth
      )
      const endY = clamp(
        Math.ceil(image.naturalHeight * (region.y + region.height)),
        startY + 1,
        image.naturalHeight
      )
      const canvas = document.createElement('canvas')
      canvas.width = endX - startX
      canvas.height = endY - startY
      const context = canvas.getContext('2d')
      if (!context) {
        reject(new Error('无法创建 Buff 搜索区域画布'))
        return
      }
      context.drawImage(
        image,
        startX,
        startY,
        canvas.width,
        canvas.height,
        0,
        0,
        canvas.width,
        canvas.height
      )
      resolve(canvas.toDataURL('image/png'))
    }
    image.onerror = () => reject(new Error('无法读取捕获预览'))
    image.src = imageUrl
  })
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value))
}

function describeStatus(activity: string, monitoring: boolean): string {
  if (!monitoring && activity === 'stopped') return '未开始'
  const labels: Record<string, string> = {
    waiting: '等待金周天',
    tracking: '20 秒计时中',
    prewarning: '即将触发',
    confirming: '等待触发确认',
    testing: '模板测试中',
    targetUnavailable: '等待游戏窗口',
    error: '运行异常',
    stopped: '已停止'
  }
  return labels[activity] ?? '未知状态'
}

function toMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason)
}
