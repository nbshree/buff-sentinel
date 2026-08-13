import { Maximize2 } from 'lucide-react'

import { Button } from '../../components/ui/button'

type EditorExpandButtonProps = {
  label: string
  onClick: () => void
}

export function EditorExpandButton({ label, onClick }: EditorExpandButtonProps) {
  return (
    <Button
      className="buff-editor-expand-action"
      size="compact"
      type="button"
      variant="outline"
      onClick={onClick}
    >
      <Maximize2 aria-hidden="true" />
      {label}
    </Button>
  )
}
