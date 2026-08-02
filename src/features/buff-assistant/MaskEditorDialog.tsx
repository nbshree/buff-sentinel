import { useEffect, useState } from 'react'

import { Button } from '../../components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '../../components/ui/dialog'
import type { NormalizedRect } from '../../lib/macro-api'
import { cloneMaskHistory, createMaskHistory, MaskEditor, type MaskHistory } from './MaskEditor'

type MaskEditorDialogProps = {
  open: boolean
  imageUrl: string
  crop: NormalizedRect
  value: MaskHistory
  onApply: (history: MaskHistory) => void
  onOpenChange: (open: boolean) => void
}

export function MaskEditorDialog({
  open,
  imageUrl,
  crop,
  value,
  onApply,
  onOpenChange
}: MaskEditorDialogProps) {
  const [draft, setDraft] = useState<MaskHistory>(() => createMaskHistory())

  useEffect(() => {
    if (open) setDraft(cloneMaskHistory(value))
  }, [open, value])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="buff-image-editor-dialog">
        <DialogHeader>
          <DialogTitle>精调忽略区域</DialogTitle>
          <DialogDescription>
            在倒计时数字、层数或动态闪光上涂抹。一次按下到松开算一笔，可用 Ctrl+Z 撤销。
          </DialogDescription>
        </DialogHeader>
        <div className="buff-image-editor-dialog__workspace">
          <MaskEditor expanded crop={crop} imageUrl={imageUrl} value={draft} onChange={setDraft} />
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            取消
          </Button>
          <Button
            onClick={() => {
              onApply(cloneMaskHistory(draft))
              onOpenChange(false)
            }}
          >
            应用遮罩
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
