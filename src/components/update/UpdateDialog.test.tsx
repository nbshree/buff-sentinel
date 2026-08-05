import { screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import type { AppUpdaterController } from '../../hooks/useAppUpdater'
import { renderWithUiProviders } from '../../test/test-utils'
import { UpdateDialog } from './UpdateDialog'

function createUpdater(overrides: Partial<AppUpdaterController> = {}): AppUpdaterController {
  return {
    open: true,
    status: 'available',
    currentVersion: '1.7.1',
    update: {
      version: '1.8.0',
      notes: '新增在线更新功能',
      publishedAt: '2026-07-21T12:00:00Z'
    },
    downloaded: 0,
    total: null,
    progressPercent: null,
    error: null,
    installBlockedReason: null,
    retryBlockedReason: null,
    isBusy: false,
    checkOnStartup: vi.fn().mockResolvedValue(undefined),
    checkForUpdate: vi.fn().mockResolvedValue(undefined),
    installUpdate: vi.fn().mockResolvedValue(undefined),
    retry: vi.fn().mockResolvedValue(undefined),
    setOpen: vi.fn(),
    ...overrides
  }
}

describe('UpdateDialog', () => {
  it('renders release notes as plain text', () => {
    const updater = createUpdater({
      update: {
        version: '1.8.0',
        notes: '<img src=x onerror="window.evil()">\n修复若干问题',
        publishedAt: null
      }
    })

    renderWithUiProviders(<UpdateDialog updater={updater} />)

    expect(screen.getByText(/<img src=x onerror=/)).toBeInTheDocument()
    expect(document.querySelector('.update-dialog__notes img')).not.toBeInTheDocument()
    expect(screen.getByRole('region', { name: '更新内容' })).toHaveAttribute('tabindex', '0')
  })

  it('keeps the update icon and title in a horizontal heading container', () => {
    renderWithUiProviders(<UpdateDialog updater={createUpdater()} />)

    const title = screen.getByRole('heading', { name: '发现新版本 v1.8.0' })
    expect(title.closest('[data-slot="dialog-header"]')?.parentElement).toHaveClass(
      'update-dialog__heading'
    )
  })

  it.each([
    '宏正在执行，请先停止执行再安装更新。',
    '当前有未保存的编辑，请先保存或撤销后再安装更新。'
  ])('explains why installation is disabled: %s', (installBlockedReason) => {
    renderWithUiProviders(<UpdateDialog updater={createUpdater({ installBlockedReason })} />)

    expect(screen.getByText(installBlockedReason)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '立即更新' })).toBeDisabled()
  })

  it('shows cumulative download progress', () => {
    renderWithUiProviders(
      <UpdateDialog
        updater={createUpdater({
          status: 'downloading',
          downloaded: 25 * 1024 * 1024,
          total: 100 * 1024 * 1024,
          progressPercent: 25,
          isBusy: true
        })}
      />
    )

    expect(screen.getByRole('progressbar', { name: '更新下载进度' })).toHaveAttribute(
      'aria-valuenow',
      '25'
    )
    expect(screen.getByText('25%')).toBeInTheDocument()
    expect(screen.getByText('25.0 MB / 100.0 MB')).toBeInTheDocument()
  })

  it('shows a completed download when installation starts without a known total', () => {
    renderWithUiProviders(
      <UpdateDialog
        updater={createUpdater({
          status: 'installing',
          downloaded: 7 * 1024 * 1024,
          total: null,
          progressPercent: null,
          isBusy: true
        })}
      />
    )

    expect(screen.getByText('下载完成')).toBeInTheDocument()
    expect(screen.queryByText('下载中')).not.toBeInTheDocument()
    expect(screen.getByRole('progressbar', { name: '更新下载进度' })).toHaveAttribute(
      'aria-valuenow',
      '100'
    )
  })

  it('offers retry after an update error', () => {
    const updater = createUpdater({
      status: 'error',
      update: null,
      error: '检查更新失败：网络不可用'
    })

    renderWithUiProviders(<UpdateDialog updater={updater} />)

    expect(screen.getByText('检查更新失败：网络不可用')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '重试' })).toBeInTheDocument()
  })

  it('explains and disables an installation retry while the macro is busy', () => {
    const retryBlockedReason = '宏正在执行，请先停止执行再安装更新。'
    const updater = createUpdater({
      status: 'error',
      error: '安装更新失败：宏正在执行。',
      retryBlockedReason
    })

    renderWithUiProviders(<UpdateDialog updater={updater} />)

    expect(screen.getByText(retryBlockedReason)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '重试' })).toBeDisabled()
  })
})
