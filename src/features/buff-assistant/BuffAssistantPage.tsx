import {
  Activity,
  CheckCircle2,
  ChevronDown,
  Clock3,
  Crosshair,
  ExternalLink,
  CircleHelp,
  Eye,
  ImagePlus,
  MonitorPlay,
  Pencil,
  Play,
  Plus,
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
import { Checkbox } from '../../components/ui/checkbox'
import { Input } from '../../components/ui/input'
import { Slider } from '../../components/ui/slider'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '../../components/ui/select'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger
} from '../../components/ui/dialog'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger
} from '../../components/ui/tooltip'
import type { BuffAssistantController } from '../../hooks/useBuffAssistantController'
import type {
  BuffGlobalSettings,
  BuffListenerConfig,
  BuffListenerSettings,
  BuffOverlayPreviewMode,
  BuffSoundCue,
  BuffSoundSource,
  BuffSoundTemplateSummary,
  NormalizedRect
} from '../../lib/buff-sentinel-api'
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
const overlayPreviewOptions: Array<{ value: BuffOverlayPreviewMode; label: string }> = [
  { value: 'waiting', label: '等待监听' },
  { value: 'countdown', label: '倒计时' },
  { value: 'confirming', label: '等待确认' },
  { value: 'targetUnavailable', label: '等待游戏窗口' }
]
const defaultListenerSettings: BuffListenerSettings = {
  cycleMs: 20_000,
  deadlineGraceMs: 1500,
  matchMode: 'pixel',
  threshold: 0.95,
  confirmFrames: 3,
  missingFrames: 5,
  sound: {
    triggerEnabled: true,
    prewarnThreeEnabled: true,
    prewarnTwoEnabled: true,
    prewarnOneEnabled: true,
    triggerSource: { type: 'template', templateId: 'template-1' },
    prewarnThreeSource: { type: 'template', templateId: 'template-1' },
    prewarnTwoSource: { type: 'template', templateId: 'template-1' },
    prewarnOneSource: { type: 'template', templateId: 'template-1' },
    volume: 0.45
  }
}

export function BuffAssistantPage({ controller }: BuffAssistantPageProps) {
  const {
    state,
    windows,
    preview,
    metrics,
    logs,
    busy,
    error,
    refreshWindows,
    capturePreview,
    updateBuffSearchRegion,
    getListenerTemplate,
    saveListener,
    updateListener,
    deleteListener,
    updateSettings,
    requestBorderlessCaptureAccess,
    startMonitor,
    stopMonitor,
    startTest,
    stopTest,
    clearLogs,
    setOverlayEditing,
    setOverlayPreview
  } = controller
  const [selectedWindowId, setSelectedWindowId] = useState('')
  const [searchRegion, setSearchRegion] = useState<NormalizedRect | null>(null)
  const [templateSource, setTemplateSource] = useState<string | null>(null)
  const [savedTemplateSource, setSavedTemplateSource] = useState<string | null>(null)
  const [usingSavedTemplate, setUsingSavedTemplate] = useState(false)
  const [editingFromSharedSource, setEditingFromSharedSource] = useState(false)
  const [loadingListenerTemplate, setLoadingListenerTemplate] = useState(false)
  const [templateCrop, setTemplateCrop] = useState<NormalizedRect | null>(null)
  const [maskHistory, setMaskHistory] = useState<MaskHistory>(() => createMaskHistory())
  const [searchRegionEditorOpen, setSearchRegionEditorOpen] = useState(false)
  const [templateCropEditorOpen, setTemplateCropEditorOpen] = useState(false)
  const [maskEditorOpen, setMaskEditorOpen] = useState(false)
  const [settingsDialogOpen, setSettingsDialogOpen] = useState(false)
  const [settings, setSettings] = useState<BuffGlobalSettings>(state.config.settings)
  const [listenerDialogOpen, setListenerDialogOpen] = useState(false)
  const [editingListenerId, setEditingListenerId] = useState<string | null>(null)
  const [listenerName, setListenerName] = useState('')
  const [listenerEnabled, setListenerEnabled] = useState(true)
  const [listenerSettings, setListenerSettings] =
    useState<BuffListenerSettings>(defaultListenerSettings)
  const [listenerError, setListenerError] = useState<string | null>(null)
  const [overlayEditing, setOverlayEditingState] = useState(false)
  const [overlayPreviewMode, setOverlayPreviewMode] =
    useState<BuffOverlayPreviewMode>('countdown')
  const [soundTemplates, setSoundTemplates] = useState<BuffSoundTemplateSummary[]>([])
  const [soundError, setSoundError] = useState<string | null>(null)
  const [uploadingCue, setUploadingCue] = useState<BuffSoundCue | null>(null)
  const [logsCollapsed, setLogsCollapsed] = useState(false)
  const maskRef = useRef<MaskEditorHandle>(null)
  const listenerTemplateRequestRef = useRef(0)

  useEffect(() => {
    if (!settingsDialogOpen) setSettings(state.config.settings)
  }, [settingsDialogOpen, state.config.settings])

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
      setListenerSettings((current) => ({
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
      await window.api.playBuffAssistantSound(cue, source, listenerSettings.sound.volume)
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

  const configurationLocked = state.isMonitoring || state.activity === 'testing'
  const canStart = Boolean(
    state.config.target &&
    state.config.searchRegion &&
    state.config.listeners.some((listener) => listener.enabled && listener.template)
  )
  const enabledListenerCount = state.config.listeners.filter((listener) => listener.enabled).length
  const monitoringStatus = state.isMonitoring
    ? { label: '监控中', detail: `${enabledListenerCount} 个监听项运行中`, tone: 'active' }
    : state.activity === 'testing'
      ? { label: '测试中', detail: '正在校验监听图标', tone: 'testing' }
      : overlayEditing
        ? { label: '调整浮窗', detail: '拖动并保存悬浮窗位置', tone: 'editing' }
        : canStart
          ? { label: '待命', detail: `${enabledListenerCount} 个监听项已就绪`, tone: 'ready' }
          : { label: '未就绪', detail: '完成捕获与监听配置后开始', tone: 'idle' }
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
    if (state.config.listeners.some((listener) => listener.template)) {
      void updateBuffSearchRegion(region).catch(() => undefined)
    }
  }

  function handleTemplateCropChange(crop: NormalizedRect): void {
    setTemplateCrop(crop)
    setMaskHistory(createMaskHistory())
  }

  function openAddListener(): void {
    listenerTemplateRequestRef.current += 1
    setEditingListenerId(null)
    setListenerName(`监听图标 ${state.config.listeners.length + 1}`)
    setListenerEnabled(true)
    setListenerSettings(defaultListenerSettings)
    setTemplateCrop(null)
    setSavedTemplateSource(null)
    setUsingSavedTemplate(false)
    setEditingFromSharedSource(false)
    setLoadingListenerTemplate(false)
    setMaskHistory(createMaskHistory())
    setListenerError(null)
    setListenerDialogOpen(true)
  }

  async function openEditListener(listener: BuffListenerConfig): Promise<void> {
    const requestId = listenerTemplateRequestRef.current + 1
    listenerTemplateRequestRef.current = requestId
    setEditingListenerId(listener.id)
    setListenerName(listener.name)
    setListenerEnabled(listener.enabled)
    setListenerSettings(listener.settings)
    setTemplateCrop(null)
    setSavedTemplateSource(null)
    setUsingSavedTemplate(false)
    setEditingFromSharedSource(false)
    setMaskHistory(createMaskHistory())
    setListenerError(null)
    setListenerDialogOpen(true)
    if (!listener.template) return
    setLoadingListenerTemplate(true)
    try {
      const template = await getListenerTemplate(listener.id)
      if (listenerTemplateRequestRef.current !== requestId) return
      setSavedTemplateSource(template.sourceDataUrl ?? template.imageDataUrl)
      setUsingSavedTemplate(true)
      setEditingFromSharedSource(false)
      setTemplateCrop(
        template.sourceDataUrl && template.crop
          ? template.crop
          : { x: 0, y: 0, width: 1, height: 1 }
      )
      setMaskHistory(createMaskHistory(template.maskDataUrl))
    } catch (reason) {
      if (listenerTemplateRequestRef.current === requestId) setListenerError(toMessage(reason))
    } finally {
      if (listenerTemplateRequestRef.current === requestId) setLoadingListenerTemplate(false)
    }
  }

  function startListenerRecrop(): void {
    setUsingSavedTemplate(false)
    setEditingFromSharedSource(true)
    setTemplateCrop(null)
    setMaskHistory(createMaskHistory())
  }

  async function handleSaveListener(): Promise<void> {
    const name = listenerName.trim()
    if (!name) {
      setListenerError('请输入监听项名称')
      return
    }
    if (name.length > 20) {
      setListenerError('监听项名称不能超过 20 个字符')
      return
    }
    try {
      if (editingListenerId && usingSavedTemplate && !editingFromSharedSource) {
        await updateListener(
          editingListenerId,
          name,
          listenerEnabled,
          listenerSettings,
          maskRef.current?.getMaskDataUrl()
        )
      } else if (templateCrop && searchRegion) {
        await saveListener(
          editingListenerId,
          name,
          listenerEnabled,
          listenerSettings,
          searchRegion,
          templateCrop,
          maskRef.current?.getMaskDataUrl()
        )
      } else if (editingListenerId) {
        await updateListener(editingListenerId, name, listenerEnabled, listenerSettings)
      } else {
        setListenerError('请先从预览中框选监听图标')
        return
      }
      setListenerDialogOpen(false)
    } catch (reason) {
      setListenerError(toMessage(reason))
    }
  }

  const listenerEditorSource =
    usingSavedTemplate && !editingFromSharedSource ? savedTemplateSource : templateSource

  async function handleOverlayEdit(): Promise<void> {
    const next = !overlayEditing
    await setOverlayEditing(next)
    setOverlayEditingState(next)
    if (next) setOverlayPreviewMode('countdown')
  }

  async function handleOverlayPreviewChange(mode: BuffOverlayPreviewMode): Promise<void> {
    const previous = overlayPreviewMode
    setOverlayPreviewMode(mode)
    try {
      await setOverlayPreview(mode)
    } catch {
      setOverlayPreviewMode(previous)
    }
  }

  async function handleHideSystemCaptureBorderChange(hidden: boolean): Promise<void> {
    if (!hidden) {
      setSettings((current) => ({
        ...current,
        capture: { ...current.capture, showSystemBorder: true }
      }))
      return
    }
    const result = await requestBorderlessCaptureAccess()
    if (result !== 'allowed') return
    setSettings((current) => ({
      ...current,
      capture: { ...current.capture, showSystemBorder: false }
    }))
  }

  function handleSettingsDialogOpenChange(open: boolean): void {
    if (open) {
      setSettings(state.config.settings)
      setSoundError(null)
    }
    setSettingsDialogOpen(open)
  }

  async function handleSaveSettings(): Promise<void> {
    await updateSettings(settings)
    setSettingsDialogOpen(false)
  }

  return (
    <TooltipProvider>
      <div className="buff-assistant-page">
        {error || state.lastError ? (
          <div className="buff-assistant-error" role="alert">
            {error ?? state.lastError}
          </div>
        ) : null}

        <aside className="buff-sidebar" aria-label="捕获配置">
          <section className="buff-card buff-sidebar-section buff-template-wizard">
            <header>
              <div>
                <Crosshair aria-hidden="true" />
                <div>
                  <span className="buff-section-kicker">CAPTURE SOURCE</span>
                  <h3>共享捕获区域</h3>
                  <p>所有监听图标共用同一个游戏窗口与搜索区域。</p>
                </div>
              </div>
              <Button
                aria-label="刷新窗口"
                disabled={busy || configurationLocked}
                size="icon-sm"
                title="刷新窗口"
                variant="outline"
                onClick={() => void refreshWindows()}
              >
                <RefreshCw aria-hidden="true" />
              </Button>
            </header>

            <div className="buff-window-field">
              <span id="buff-target-window-label">目标游戏窗口</span>
              <Select
                disabled={configurationLocked || windows.length === 0}
                value={selectedWindowId}
                onValueChange={setSelectedWindowId}
              >
                <SelectTrigger
                  aria-labelledby="buff-target-window-label"
                  className="buff-window-select"
                >
                  <SelectValue placeholder="没有可捕获窗口" />
                </SelectTrigger>
                <SelectContent>
                  {windows.map((candidate) => (
                    <SelectItem key={candidate.id} value={candidate.id}>
                      {candidate.processName} · {candidate.windowTitle} · {candidate.width}×
                      {candidate.height}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <Button
              className="buff-preview-button"
              disabled={busy || configurationLocked || !selectedWindowId}
              variant="outline"
              onClick={() => void handlePreview()}
            >
              <Eye aria-hidden="true" />
              捕获预览
            </Button>

            {preview ? (
              <div className="buff-wizard-step">
                <div className="buff-wizard-step__title">
                  <span>1</span>
                  <div>
                    <strong>框选 Buff 栏搜索区域</strong>
                    <p>区域越小识别越快，请覆盖全部图标可能出现的位置。</p>
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
            ) : (
              <div className="buff-capture-empty">
                <div className="buff-capture-empty__icon">
                  <ImagePlus aria-hidden="true" />
                </div>
                <strong>等待捕获预览</strong>
                <p>选择游戏窗口后生成画面，再框选 Buff 栏。</p>
              </div>
            )}
          </section>
        </aside>

        <main className="buff-dashboard">
          <section className="buff-card buff-monitor-toolbar">
            <div className="buff-monitor-status">
              <div
                className="buff-monitor-status__indicator"
                data-tone={monitoringStatus.tone}
                aria-hidden="true"
              >
                <Activity />
              </div>
              <div>
                <span className="buff-section-kicker">SENTINEL STATUS</span>
                <div className="buff-monitor-status__heading">
                  <h2>{monitoringStatus.label}</h2>
                  <span className="buff-status-badge" data-tone={monitoringStatus.tone}>
                    <span aria-hidden="true" />
                    {monitoringStatus.label}
                  </span>
                </div>
                <p>{monitoringStatus.detail}</p>
              </div>
            </div>
            <div className="buff-card__actions buff-monitor-actions">
              {state.isMonitoring ? (
                <Button disabled={busy} variant="destructive" onClick={() => void stopMonitor()}>
                  <Square aria-hidden="true" />
                  停止监控
                </Button>
              ) : (
                <Button
                  disabled={busy || !canStart || overlayEditing}
                  title={overlayEditing ? '请先保存悬浮位置' : undefined}
                  onClick={() => void startMonitor()}
                >
                  <Play aria-hidden="true" />
                  开始监控
                </Button>
              )}
              <Button disabled={busy} variant="outline" onClick={() => void handleOverlayEdit()}>
                <MonitorPlay aria-hidden="true" />
                {overlayEditing ? '保存悬浮位置' : '调整悬浮位置'}
              </Button>
              {overlayEditing ? (
                <label className="buff-overlay-preview-control" htmlFor="buff-overlay-preview-mode">
                  <span>预览状态</span>
                  <Select
                    disabled={busy}
                    value={overlayPreviewMode}
                    onValueChange={(value) =>
                      void handleOverlayPreviewChange(value as BuffOverlayPreviewMode)
                    }
                  >
                    <SelectTrigger
                      id="buff-overlay-preview-mode"
                      aria-label="悬浮窗预览状态"
                      className="buff-overlay-preview-control__trigger"
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {overlayPreviewOptions.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </label>
              ) : null}
              <Dialog open={settingsDialogOpen} onOpenChange={handleSettingsDialogOpenChange}>
                <DialogTrigger asChild>
                  <Button disabled={busy || configurationLocked} variant="outline">
                    <Settings2 aria-hidden="true" />
                    识别与提醒设置
                  </Button>
                </DialogTrigger>
                <DialogContent className="max-h-[calc(100vh-48px)] grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0 sm:max-w-[1000px]">
                  <DialogHeader className="border-b border-border px-5 py-4 pr-14">
                    <DialogTitle>识别与提醒设置</DialogTitle>
                    <DialogDescription>调整浮窗与系统捕获参数，点击保存后生效。</DialogDescription>
                  </DialogHeader>
                  <div className="buff-settings-dialog">
                    <div className="buff-global-settings-row">
                      <label>
                        <span>浮窗配色</span>
                        <Select
                          aria-label="浮窗配色"
                          value={settings.overlay.colorScheme}
                          onValueChange={(value) =>
                            setSettings((current) => ({
                              ...current,
                              overlay: {
                                ...current.overlay,
                                colorScheme: value as BuffGlobalSettings['overlay']['colorScheme']
                              }
                            }))
                          }
                        >
                          <SelectTrigger>
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectItem value="blackWhite">黑底白字（默认）</SelectItem>
                            <SelectItem value="gold">金色</SelectItem>
                          </SelectContent>
                        </Select>
                      </label>
                      <div className="buff-check-row">
                        <Checkbox
                          aria-labelledby="exclude-overlay-capture-label"
                          checked={settings.overlay.excludeFromCapture}
                          disabled={busy}
                          id="exclude-overlay-capture"
                          onCheckedChange={(checked) =>
                            setSettings((current) => ({
                              ...current,
                              overlay: {
                                ...current.overlay,
                                excludeFromCapture: checked === true
                              }
                            }))
                          }
                        />
                        <div className="buff-check-label">
                          <label
                            id="exclude-overlay-capture-label"
                            htmlFor="exclude-overlay-capture"
                          >
                            排除录屏捕获
                          </label>
                          <SettingTooltip
                            label="查看排除录屏捕获说明"
                            content="开启后，OBS 等使用系统捕获接口的工具通常不会录入 Buff 悬浮窗；游戏捕获、驱动级采集和采集卡可能不受支持。"
                          />
                        </div>
                      </div>
                      <div className="buff-check-row">
                        <Checkbox
                          aria-labelledby="hide-system-capture-border-label"
                          checked={!settings.capture.showSystemBorder}
                          disabled={busy || !state.captureBorderSupported}
                          id="hide-system-capture-border"
                          onCheckedChange={(checked) =>
                            void handleHideSystemCaptureBorderChange(checked === true)
                          }
                        />
                        <div className="buff-check-label">
                          <label
                            id="hide-system-capture-border-label"
                            htmlFor="hide-system-capture-border"
                          >
                            隐藏系统捕获黄色边框
                          </label>
                          <SettingTooltip
                            label="查看隐藏系统捕获黄色边框说明"
                            content={
                              state.captureBorderSupported
                                ? '隐藏时 Windows 可能请求授权；该边框由 Windows Graphics Capture 绘制，不会进入捕获画面。'
                                : '当前 Windows 版本不支持隐藏系统捕获黄色边框。'
                            }
                          />
                        </div>
                      </div>
                    </div>
                    <div className="buff-sound-options">
                      {state.captureBorderNotice ? (
                        <p className="buff-setting-help buff-setting-help--warning">
                          {state.captureBorderNotice}
                        </p>
                      ) : null}
                    </div>
                  </div>
                  <DialogFooter className="border-t border-border px-5 py-4">
                    <Button
                      disabled={busy}
                      variant="outline"
                      onClick={() => setSettingsDialogOpen(false)}
                    >
                      取消
                    </Button>
                    <Button disabled={busy} onClick={() => void handleSaveSettings()}>
                      <Save aria-hidden="true" />
                      保存设置
                    </Button>
                  </DialogFooter>
                </DialogContent>
              </Dialog>
            </div>
          </section>

          <section className="buff-card buff-listener-section">
            <header>
              <div>
                <MonitorPlay aria-hidden="true" />
                <div>
                  <span className="buff-section-kicker">ACTIVE LISTENERS</span>
                  <h3>监听图标</h3>
                  <p>已添加 {state.config.listeners.length}/8 个，启用项会同时监听。</p>
                </div>
              </div>
              <Button
                className="buff-add-listener-button"
                disabled={
                  busy ||
                  configurationLocked ||
                  !templateSource ||
                  state.config.listeners.length >= 8
                }
                size="sm"
                onClick={openAddListener}
              >
                <Plus aria-hidden="true" />
                添加监听图标
              </Button>
            </header>
            {state.config.listeners.length === 0 ? (
              <div className="buff-listener-empty">
                捕获预览并框选共享搜索区域后，即可添加第一个监听图标。
              </div>
            ) : (
              <div className="buff-listener-list">
                {state.config.listeners.map((listener) => {
                  const metric = metrics[listener.id]
                  const runtime = state.listeners.find((item) => item.id === listener.id)
                  const confidence = Math.round(
                    (metric?.confidence ?? runtime?.lastConfidence ?? 0) * 100
                  )
                  const activity = runtime?.activity ?? 'stopped'
                  return (
                    <article
                      className="buff-listener-item"
                      data-enabled={listener.enabled}
                      key={listener.id}
                    >
                      <div className="buff-listener-item__topline">
                        <div
                          className="buff-listener-thumbnail"
                          data-configured={Boolean(listener.template)}
                          aria-hidden="true"
                        >
                          {listener.template ? (
                            <>
                              <CheckCircle2 />
                              <small>
                                {listener.template.width}×{listener.template.height}
                              </small>
                            </>
                          ) : (
                            <ImagePlus />
                          )}
                        </div>
                        <div className="buff-listener-identity">
                          <label className="buff-listener-toggle">
                            <Checkbox
                              aria-label={listener.name}
                              checked={listener.enabled}
                              disabled={busy || configurationLocked}
                              onCheckedChange={(checked) =>
                                void updateListener(
                                  listener.id,
                                  listener.name,
                                  checked === true,
                                  listener.settings
                                )
                              }
                            />
                            <span>{listener.name}</span>
                          </label>
                          <span>{listener.template ? '模板已配置' : '需要重新裁剪模板'}</span>
                        </div>
                        <span className="buff-runtime-badge" data-activity={activity}>
                          {runtimeActivityLabel(activity)}
                        </span>
                      </div>
                      <div className="buff-listener-metrics">
                        <div>
                          <span>实时置信度</span>
                          <strong>{confidence}%</strong>
                        </div>
                        <div>
                          <span>触发阈值</span>
                          <strong>{Math.round(listener.settings.threshold * 100)}%</strong>
                        </div>
                        <div>
                          <span>监听周期</span>
                          <strong>{Math.round(listener.settings.cycleMs / 1000)}s</strong>
                        </div>
                      </div>
                      <div className="buff-confidence-track" aria-hidden="true">
                        <span style={{ width: `${confidence}%` }} />
                      </div>
                      <div className="buff-card__actions buff-listener-actions">
                        {state.activity === 'testing' && runtime?.activity === 'testing' ? (
                          <Button
                            disabled={busy}
                            size="sm"
                            variant="outline"
                            onClick={() => void stopTest()}
                          >
                            停止测试
                          </Button>
                        ) : (
                          <Button
                            disabled={
                              busy || configurationLocked || !listener.template || !selectedWindowId
                            }
                            size="sm"
                            variant="outline"
                            onClick={() => void startTest(selectedWindowId, listener.id)}
                          >
                            测试
                          </Button>
                        )}
                        <Button
                          disabled={busy || configurationLocked}
                          size="sm"
                          variant="outline"
                          onClick={() => void openEditListener(listener)}
                        >
                          <Pencil aria-hidden="true" />
                          编辑
                        </Button>
                        <Button
                          aria-label={`删除${listener.name}`}
                          disabled={busy || configurationLocked}
                          size="icon-compact"
                          variant="destructive"
                          onClick={() => {
                            if (window.confirm(`确定删除监听项“${listener.name}”吗？`)) {
                              void deleteListener(listener.id)
                            }
                          }}
                        >
                          <Trash2 aria-hidden="true" />
                        </Button>
                      </div>
                    </article>
                  )
                })}
              </div>
            )}
          </section>

          <section
            className="buff-execution-log"
            data-collapsed={logsCollapsed}
            aria-labelledby="buff-execution-log-title"
          >
            <header>
              <div>
                <ScrollText aria-hidden="true" />
                <div>
                  <span className="buff-section-kicker">ACTIVITY STREAM</span>
                  <h3 id="buff-execution-log-title">执行日志</h3>
                </div>
                <span className="buff-log-count">{logs.length}</span>
              </div>
              <div className="buff-log-actions">
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
                <Button
                  aria-expanded={!logsCollapsed}
                  aria-label={logsCollapsed ? '展开执行日志' : '收起执行日志'}
                  className="buff-log-toggle"
                  size="icon-compact"
                  title={logsCollapsed ? '展开日志' : '收起日志'}
                  type="button"
                  variant="outline"
                  onClick={() => setLogsCollapsed((collapsed) => !collapsed)}
                >
                  <ChevronDown aria-hidden="true" />
                </Button>
              </div>
            </header>
            <div className="buff-execution-log__body" aria-live="polite">
              {logs.length === 0 ? (
                <p className="buff-execution-log__empty">
                  <Clock3 aria-hidden="true" />
                  暂无日志，开始监控后将在这里显示活动。
                </p>
              ) : (
                logs.map((item, index) => (
                  <p
                    className={`buff-log-line buff-log-line--${logTone(item)}`}
                    key={`${index}-${item}`}
                  >
                    {item}
                  </p>
                ))
              )}
            </div>
          </section>
        </main>

        <Dialog open={listenerDialogOpen} onOpenChange={setListenerDialogOpen}>
          <DialogContent className="max-h-[calc(100vh-48px)] grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden p-0 sm:max-w-[1000px]">
            <DialogHeader className="border-b border-border px-5 py-4 pr-14">
              <DialogTitle>{editingListenerId ? '编辑监听图标' : '添加监听图标'}</DialogTitle>
              <DialogDescription>
                名称、模板、识别参数、周期和提示音均仅作用于当前项。
              </DialogDescription>
            </DialogHeader>
            <div className="buff-listener-dialog">
              <div className="buff-listener-basics">
                <label htmlFor="buff-listener-name">
                  <span>名称</span>
                  <Input
                    autoComplete="off"
                    id="buff-listener-name"
                    maxLength={20}
                    placeholder="请输入监听项名称"
                    type="text"
                    value={listenerName}
                    onChange={(event) => setListenerName(event.target.value)}
                  />
                </label>
                <label className="buff-listener-enabled">
                  <Checkbox
                    aria-label="启用监听"
                    checked={listenerEnabled}
                    onCheckedChange={(checked) => setListenerEnabled(checked === true)}
                  />
                  启用监听
                </label>
              </div>
              {loadingListenerTemplate ? (
                <p className="buff-listener-template-loading">正在加载已保存的图标和遮罩…</p>
              ) : listenerEditorSource ? (
                <div className="buff-listener-template-editor">
                  {usingSavedTemplate && templateSource ? (
                    <Button type="button" variant="outline" onClick={startListenerRecrop}>
                      <ImagePlus aria-hidden="true" />
                      从当前 Buff 区域刷新监听图标
                    </Button>
                  ) : null}
                  <div className="buff-wizard-step__title">
                    <span>1</span>
                    <div>
                      <strong>裁剪图标主体</strong>
                      <p>
                        {usingSavedTemplate
                        ? editingFromSharedSource
                          ? '当前正在使用最新 Buff 区域，可重新框选并保存为新的监听图标。'
                          : '当前显示已保存的监听图标；如需更新，请点击下方刷新按钮。'
                        : '只框选当前图标，不要包含相邻 Buff.'}
                      </p>
                    </div>
                  </div>
                  <RegionSelector
                    imageUrl={listenerEditorSource}
                    label={listenerName || '监听图标'}
                    upscaleSmallImage={usingSavedTemplate && !editingFromSharedSource}
                    value={templateCrop}
                    onChange={handleTemplateCropChange}
                    onRequestExpand={() => setTemplateCropEditorOpen(true)}
                  />
                  {templateCrop ? (
                    <MaskEditor
                      crop={templateCrop}
                      imageUrl={listenerEditorSource}
                      ref={maskRef}
                      value={maskHistory}
                      onChange={setMaskHistory}
                      onRequestExpand={() => setMaskEditorOpen(true)}
                    />
                  ) : null}
                </div>
              ) : null}
              <ListenerSettingsEditor
                settings={listenerSettings}
                soundError={soundError}
                soundTemplates={soundTemplates}
                uploadingCue={uploadingCue}
                onChange={setListenerSettings}
                onOpenTts={() => void openTtsOnline()}
                onPreviewSound={(cue, source) => void previewSound(cue, source)}
                onUploadSound={(cue, field) => void importSound(cue, field)}
              />
              {listenerError ? (
                <p className="buff-sound-error" role="alert">
                  {listenerError}
                </p>
              ) : null}
            </div>
            <DialogFooter className="border-t border-border px-5 py-4">
              <Button variant="outline" onClick={() => setListenerDialogOpen(false)}>
                取消
              </Button>
              <Button
                disabled={busy || loadingListenerTemplate}
                onClick={() => void handleSaveListener()}
              >
                <Save aria-hidden="true" />
                保存监听项
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>

        {preview ? (
          <RegionEditorDialog
            description="框内拖动可整体移动，拖动四边或四角可精确调整搜索范围。"
            imageUrl={preview.dataUrl}
            label="Buff 搜索区域"
            open={searchRegionEditorOpen}
            title="精调 Buff 栏搜索区域"
            value={searchRegion}
            onApply={handleSearchRegionChange}
            onOpenChange={setSearchRegionEditorOpen}
          />
        ) : null}

        {listenerEditorSource ? (
          <RegionEditorDialog
            description="框内拖动可整体移动，拖动四边或四角可贴合监听图标主体。"
            imageUrl={listenerEditorSource}
            label={listenerName || '监听图标'}
            open={templateCropEditorOpen}
            title="精调监听图标主体"
            value={templateCrop}
            warning={
              maskHistory.present.length > 0 || maskHistory.baseMaskDataUrl
                ? '应用新的图标范围后，将清空已涂抹的忽略区域。'
                : undefined
            }
            onApply={handleTemplateCropChange}
            onOpenChange={setTemplateCropEditorOpen}
          />
        ) : null}

        {listenerEditorSource && templateCrop ? (
          <MaskEditorDialog
            crop={templateCrop}
            imageUrl={listenerEditorSource}
            open={maskEditorOpen}
            value={maskHistory}
            onApply={setMaskHistory}
            onOpenChange={setMaskEditorOpen}
          />
        ) : null}
      </div>
    </TooltipProvider>
  )
}

type ListenerSettingsEditorProps = {
  settings: BuffListenerSettings
  soundTemplates: BuffSoundTemplateSummary[]
  uploadingCue: BuffSoundCue | null
  soundError: string | null
  onChange: (settings: BuffListenerSettings) => void
  onPreviewSound: (cue: BuffSoundCue, source: BuffSoundSource) => void
  onUploadSound: (cue: BuffSoundCue, field: SoundSourceField) => void
  onOpenTts: () => void
}

function ListenerSettingsEditor({
  settings,
  soundTemplates,
  uploadingCue,
  soundError,
  onChange,
  onPreviewSound,
  onUploadSound,
  onOpenTts
}: ListenerSettingsEditorProps) {
  const setSound = (patch: Partial<BuffListenerSettings['sound']>) =>
    onChange({ ...settings, sound: { ...settings.sound, ...patch } })
  const setMatchMode = (matchMode: BuffListenerSettings['matchMode']) => {
    const threshold =
      matchMode === 'brightText' && settings.threshold === 0.95
        ? 0.84
        : matchMode === 'pixel' && settings.threshold === 0.84
          ? 0.95
          : settings.threshold
    onChange({ ...settings, matchMode, threshold })
  }

  return (
    <div className="buff-listener-settings">
      <div className="buff-settings-grid">
        <label>
          <span>周期（秒）</span>
          <Input
            max={120}
            min={5}
            step={0.01}
            type="number"
            value={settings.cycleMs / 1000}
            onChange={(event) =>
              onChange({ ...settings, cycleMs: Math.round(Number(event.target.value) * 1000) })
            }
          />
        </label>
        <div className="buff-settings-field">
          <label htmlFor="listener-deadline-grace-ms">
            <span className="buff-setting-label">
              触发宽限期
              <SettingTooltip label="查看触发宽限期说明" content="单位：毫秒，建议值 1500" />
            </span>
          </label>
          <Input
            id="listener-deadline-grace-ms"
            max={2000}
            min={0}
            step={50}
            type="number"
            value={settings.deadlineGraceMs}
            onChange={(event) =>
              onChange({ ...settings, deadlineGraceMs: Number(event.target.value) })
            }
          />
        </div>
        <div className="buff-settings-field">
          <label htmlFor="listener-match-mode">
            <span className="buff-setting-label">
              识别模式
              <SettingTooltip
                label="查看识别模式说明"
                content="像素图标比较固定灰度像素；亮色文字补偿半透明背景变化并比较文字轮廓。"
              />
            </span>
          </label>
          <Select value={settings.matchMode} onValueChange={setMatchMode}>
            <SelectTrigger id="listener-match-mode" aria-label="识别模式">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="pixel">像素图标</SelectItem>
              <SelectItem value="brightText">亮色文字</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <label>
          <span>匹配阈值</span>
          <Input
            max={0.99}
            min={0.5}
            step={0.01}
            type="number"
            value={settings.threshold}
            onChange={(event) => onChange({ ...settings, threshold: Number(event.target.value) })}
          />
        </label>
        <label>
          <span>确认帧数</span>
          <Input
            max={12}
            min={1}
            type="number"
            value={settings.confirmFrames}
            onChange={(event) =>
              onChange({ ...settings, confirmFrames: Number(event.target.value) })
            }
          />
        </label>
        <label>
          <span>消失帧数</span>
          <Input
            max={30}
            min={1}
            type="number"
            value={settings.missingFrames}
            onChange={(event) =>
              onChange({ ...settings, missingFrames: Number(event.target.value) })
            }
          />
        </label>
      </div>
      <div className="buff-sound-options">
        <SoundRow
          checked={settings.sound.triggerEnabled}
          cue="triggered"
          label="真实触发确认音"
          source={settings.sound.triggerSource}
          templates={soundTemplates}
          uploading={uploadingCue === 'triggered'}
          onChange={(checked) => setSound({ triggerEnabled: checked })}
          onSourceChange={(source) => setSound({ triggerSource: source })}
          onTest={() => onPreviewSound('triggered', settings.sound.triggerSource)}
          onUpload={() => onUploadSound('triggered', 'triggerSource')}
        />
        <SoundRow
          checked={settings.sound.prewarnThreeEnabled}
          cue="prewarnThree"
          label="倒计时 3 秒提示音"
          source={settings.sound.prewarnThreeSource}
          templates={soundTemplates}
          uploading={uploadingCue === 'prewarnThree'}
          onChange={(checked) => setSound({ prewarnThreeEnabled: checked })}
          onSourceChange={(source) => setSound({ prewarnThreeSource: source })}
          onTest={() => onPreviewSound('prewarnThree', settings.sound.prewarnThreeSource)}
          onUpload={() => onUploadSound('prewarnThree', 'prewarnThreeSource')}
        />
        <SoundRow
          checked={settings.sound.prewarnTwoEnabled}
          cue="prewarnTwo"
          label="倒计时 2 秒提示音"
          source={settings.sound.prewarnTwoSource}
          templates={soundTemplates}
          uploading={uploadingCue === 'prewarnTwo'}
          onChange={(checked) => setSound({ prewarnTwoEnabled: checked })}
          onSourceChange={(source) => setSound({ prewarnTwoSource: source })}
          onTest={() => onPreviewSound('prewarnTwo', settings.sound.prewarnTwoSource)}
          onUpload={() => onUploadSound('prewarnTwo', 'prewarnTwoSource')}
        />
        <SoundRow
          checked={settings.sound.prewarnOneEnabled}
          cue="prewarnOne"
          label="倒计时 1 秒提示音"
          source={settings.sound.prewarnOneSource}
          templates={soundTemplates}
          uploading={uploadingCue === 'prewarnOne'}
          onChange={(checked) => setSound({ prewarnOneEnabled: checked })}
          onSourceChange={(source) => setSound({ prewarnOneSource: source })}
          onTest={() => onPreviewSound('prewarnOne', settings.sound.prewarnOneSource)}
          onUpload={() => onUploadSound('prewarnOne', 'prewarnOneSource')}
        />
        <label className="buff-volume-row">
          <Volume2 aria-hidden="true" />
          <span>提示音量</span>
          <Slider
            aria-label="提示音量"
            max={1}
            min={0}
            step={0.05}
            value={[settings.sound.volume]}
            onValueChange={([volume]) => setSound({ volume })}
          />
          <strong>{Math.round(settings.sound.volume * 100)}%</strong>
        </label>
        <div className="buff-sound-tip">
          <p>可使用 TTS Online 生成不同监听项的语音提示。</p>
          <Button size="compact" type="button" variant="ghost" onClick={onOpenTts}>
            <ExternalLink aria-hidden="true" />
            前往 TTS Online
          </Button>
        </div>
        {soundError ? <p className="buff-sound-error">{soundError}</p> : null}
      </div>
    </div>
  )
}

function runtimeActivityLabel(activity: string): string {
  const labels: Record<string, string> = {
    stopped: '已停止',
    waiting: '等待触发',
    tracking: '倒计时中',
    prewarning: '即将触发',
    confirming: '等待确认',
    testing: '测试中',
    targetUnavailable: '等待窗口',
    error: '识别错误'
  }
  return labels[activity] ?? activity
}

function logTone(message: string): 'default' | 'warning' | 'error' | 'success' {
  const normalized = message.toLowerCase()
  if (/失败|错误|异常|error|failed/.test(normalized)) return 'error'
  if (/警告|等待|重试|warning|retry/.test(normalized)) return 'warning'
  if (/成功|已启动|已保存|完成|success|started|saved/.test(normalized)) return 'success'
  return 'default'
}

type SettingTooltipProps = {
  label: string
  content: string
}

function SettingTooltip({ label, content }: SettingTooltipProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          aria-label={label}
          className="buff-setting-tooltip-trigger"
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <CircleHelp aria-hidden="true" />
        </Button>
      </TooltipTrigger>
      <TooltipContent>{content}</TooltipContent>
    </Tooltip>
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
        <Checkbox
          aria-label={label}
          checked={checked}
          onCheckedChange={(nextChecked) => onChange(nextChecked === true)}
        />
        {label}
      </label>
      <Select
        value={value}
        onValueChange={(next) => {
          if (next === 'sine') {
            onSourceChange({ type: 'sine' })
          } else if (next.startsWith('template:')) {
            onSourceChange({ type: 'template', templateId: next.slice('template:'.length) })
          }
        }}
      >
        <SelectTrigger aria-label={`${label}来源`} className="buff-sound-row__select">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="sine">正弦波</SelectItem>
          {templates.map((template) => (
            <SelectItem key={template.id} value={`template:${template.id}`}>
              {template.name}
            </SelectItem>
          ))}
          {source.type === 'custom' ? (
            <SelectItem value={`custom:${source.assetId}`}>自定义：{source.fileName}</SelectItem>
          ) : null}
        </SelectContent>
      </Select>
      <Button
        aria-label={`上传${label} WAV`}
        className="buff-sound-row__upload"
        disabled={uploading}
        size="compact"
        type="button"
        variant="ghost"
        onClick={onUpload}
      >
        <Upload aria-hidden="true" />
        {uploading ? '选择中' : '上传'}
      </Button>
      <Button
        aria-label={`试听${label}`}
        size="compact"
        type="button"
        variant="ghost"
        onClick={onTest}
      >
        试听
      </Button>
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

function toMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason)
}
