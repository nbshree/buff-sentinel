import { CheckCircle2, LoaderCircle, MessageSquareText, Send, X } from 'lucide-react'
import { useEffect, useId, useRef, useState, type RefObject } from 'react'

import type { FeatureAccessStatus } from '../../lib/macro-api'
import { Alert, AlertDescription } from '../ui/alert'
import { Button } from '../ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '../ui/dialog'
import { Label } from '../ui/label'

type FeedbackDialogProps = {
  open: boolean
  restrictedWorkspacesUnlocked: boolean
  returnFocusRef?: RefObject<HTMLButtonElement | null>
  onOpenChange: (open: boolean) => void
  onSubmit: (content: string) => Promise<FeatureAccessStatus>
}

const confirmationDurationMs = 900

function getErrorMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message
  if (typeof error === 'string' && error.trim()) return error
  return '请稍后重试。'
}

export function FeedbackDialog({
  open,
  restrictedWorkspacesUnlocked,
  returnFocusRef,
  onOpenChange,
  onSubmit
}: FeedbackDialogProps) {
  const textareaId = useId()
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const [content, setContent] = useState('')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [confirmation, setConfirmation] = useState<string | null>(null)

  useEffect(() => {
    if (!confirmation) return

    const timeout = window.setTimeout(() => onOpenChange(false), confirmationDurationMs)
    return () => window.clearTimeout(timeout)
  }, [confirmation, onOpenChange])

  useEffect(() => {
    if (open) return

    setContent('')
    setIsSubmitting(false)
    setError(null)
    setConfirmation(null)
  }, [open])

  function close(): void {
    if (!isSubmitting) onOpenChange(false)
  }

  async function submit(): Promise<void> {
    const trimmedContent = content.trim()
    if (!trimmedContent || isSubmitting || confirmation) return

    setIsSubmitting(true)
    setError(null)
    try {
      const status = await onSubmit(content)
      const newlyUnlocked = !restrictedWorkspacesUnlocked && status.restrictedWorkspacesUnlocked
      const newlyLocked = restrictedWorkspacesUnlocked && !status.restrictedWorkspacesUnlocked
      setConfirmation(
        newlyUnlocked
          ? '已开放宏流程和游戏录制'
          : newlyLocked
            ? '已隐藏宏流程和游戏录制'
            : '感谢反馈'
      )
    } catch (nextError) {
      setError(`提交失败：${getErrorMessage(nextError)}`)
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => (nextOpen ? onOpenChange(true) : close())}>
      <DialogContent
        aria-busy={isSubmitting}
        className="feedback-dialog"
        showCloseButton={false}
        onCloseAutoFocus={(event) => {
          if (!returnFocusRef?.current) return

          event.preventDefault()
          returnFocusRef.current.focus()
        }}
        onEscapeKeyDown={(event) => {
          if (isSubmitting) event.preventDefault()
        }}
        onOpenAutoFocus={(event) => {
          event.preventDefault()
          textareaRef.current?.focus()
        }}
        onPointerDownOutside={(event) => {
          if (isSubmitting) event.preventDefault()
        }}
      >
        <header className="feedback-dialog__header">
          <span className="feedback-dialog__icon" aria-hidden="true">
            <MessageSquareText size={19} strokeWidth={1.8} />
          </span>
          <div>
            <DialogTitle>意见反馈</DialogTitle>
            <DialogDescription>欢迎留下你的建议或使用体验。</DialogDescription>
          </div>
          <Button
            aria-label="关闭意见反馈"
            className="feedback-dialog__close"
            disabled={isSubmitting}
            size="icon"
            type="button"
            variant="ghost"
            onClick={close}
          >
            <X size={18} strokeWidth={1.8} />
          </Button>
        </header>

        <div className="feedback-dialog__field">
          <Label htmlFor={textareaId}>反馈内容</Label>
          <textarea
            aria-invalid={Boolean(error) || undefined}
            disabled={isSubmitting || Boolean(confirmation)}
            id={textareaId}
            maxLength={500}
            ref={textareaRef}
            rows={5}
            value={content}
            onChange={(event) => {
              setContent(event.target.value)
              setError(null)
            }}
          />
          <span className="feedback-dialog__count" aria-label={`已输入 ${content.length} 个字符`}>
            {content.length}/500
          </span>
        </div>

        {error ? (
          <Alert className="feedback-dialog__message" variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        {confirmation ? (
          <div className="feedback-dialog__confirmation" role="status" aria-live="polite">
            <CheckCircle2 aria-hidden="true" size={18} />
            <span>{confirmation}</span>
          </div>
        ) : null}

        <footer className="feedback-dialog__footer">
          <Button
            disabled={isSubmitting || Boolean(confirmation)}
            variant="outline"
            onClick={close}
          >
            取消
          </Button>
          <Button
            aria-disabled={isSubmitting || Boolean(confirmation) || undefined}
            disabled={!content.trim() || isSubmitting || Boolean(confirmation)}
            onClick={() => void submit()}
          >
            {isSubmitting ? (
              <LoaderCircle aria-hidden="true" className="feedback-dialog__spinner" />
            ) : (
              <Send aria-hidden="true" />
            )}
            {isSubmitting ? '正在提交' : '提交反馈'}
          </Button>
        </footer>
      </DialogContent>
    </Dialog>
  )
}
