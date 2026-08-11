import { useEffect, useState } from 'react'

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '../../components/ui/dialog'
import { Button } from '../../components/ui/button'
import type { NormalizedRect } from '../../lib/buff-sentinel-api'
import { RegionSelector } from './RegionSelector'

type RegionEditorDialogProps = {
  open: boolean
  imageUrl: string
  label: string
  title: string
  description: string
  warning?: string
  value: NormalizedRect | null
  onApply: (rect: NormalizedRect) => void
  onOpenChange: (open: boolean) => void
}

export function RegionEditorDialog({
  open,
  imageUrl,
  label,
  title,
  description,
  warning,
  value,
  onApply,
  onOpenChange
}: RegionEditorDialogProps) {
  const [draft, setDraft] = useState<NormalizedRect | null>(value)

  useEffect(() => {
    if (open) setDraft(value)
  }, [open, value])

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="buff-image-editor-dialog">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
          {warning ? <p className="buff-image-editor-dialog__warning">{warning}</p> : null}
        </DialogHeader>
        <div className="buff-image-editor-dialog__workspace">
          <RegionSelector
            expanded
            imageUrl={imageUrl}
            label={label}
            value={draft}
            onChange={setDraft}
          />
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            取消
          </Button>
          <Button
            disabled={!draft}
            onClick={() => {
              if (!draft) return
              onApply(draft)
              onOpenChange(false)
            }}
          >
            应用区域
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
