import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'

import { useBuffAssistantController } from '@/hooks/useBuffAssistantController'
import { createMacroApi, installMacroApi } from '@/test/test-utils'

import { BuffAssistantPage } from './BuffAssistantPage'

function BuffAssistantHarness() {
  const controller = useBuffAssistantController()
  return <BuffAssistantPage controller={controller} />
}

describe('BuffAssistantPage', () => {
  it('saves the overlay border preference with the other settings', async () => {
    const user = userEvent.setup()
    const api = createMacroApi()
    installMacroApi(api)
    render(<BuffAssistantHarness />)

    const borderToggle = await screen.findByRole('checkbox', { name: '显示浮窗边框' })
    expect(borderToggle).toBeChecked()

    await user.click(borderToggle)
    await user.selectOptions(screen.getByRole('combobox', { name: '浮窗配色' }), 'blackWhite')
    await user.click(screen.getByRole('button', { name: '保存设置' }))

    await waitFor(() => {
      expect(api.updateBuffAssistantSettings).toHaveBeenCalledWith(
        expect.objectContaining({
          overlay: expect.objectContaining({ showBorder: false, colorScheme: 'blackWhite' })
        })
      )
    })
  })
})
