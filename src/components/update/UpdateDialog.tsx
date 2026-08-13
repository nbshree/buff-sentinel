import { CheckCircle2, Download, LoaderCircle, RefreshCw, TriangleAlert } from 'lucide-react'

import type { AppUpdaterController } from '../../hooks/useAppUpdater'
import { Button } from '../ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle
} from '../ui/dialog'

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / 1024 / 1024).toFixed(1)} MB`
}

export function UpdateDialog({ updater }: { updater: AppUpdaterController }) {
  const progress = updater.status === 'installing' ? 100 : (updater.progressPercent ?? 0)

  return (
    <Dialog open={updater.open} onOpenChange={updater.setOpen}>
      <DialogContent className="update-dialog" showCloseButton={!updater.isBusy}>
        {updater.status === 'checking' ? (
          <div className="update-dialog__state">
            <LoaderCircle className="update-dialog__spinner" />
            <DialogHeader>
              <DialogTitle>正在检查更新</DialogTitle>
              <DialogDescription>正在连接 Gitee 更新源，请稍候。</DialogDescription>
            </DialogHeader>
          </div>
        ) : null}

        {updater.status === 'upToDate' ? (
          <>
            <div className="update-dialog__state update-dialog__state--success">
              <CheckCircle2 />
              <DialogHeader>
                <DialogTitle>已经是最新版</DialogTitle>
                <DialogDescription>
                  当前版本 v{updater.currentVersion ?? '未知'}。
                </DialogDescription>
              </DialogHeader>
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={() => updater.setOpen(false)}>
                关闭
              </Button>
              <Button onClick={() => void updater.checkForUpdate()}>
                <RefreshCw />
                重新检查
              </Button>
            </DialogFooter>
          </>
        ) : null}

        {updater.status === 'available' && updater.update ? (
          <>
            <div className="update-dialog__state">
              <Download />
              <DialogHeader>
                <DialogTitle>发现新版本 v{updater.update.version}</DialogTitle>
                <DialogDescription>当前版本 v{updater.currentVersion ?? '未知'}</DialogDescription>
              </DialogHeader>
            </div>
            <section className="update-dialog__notes">
              <h3>更新内容</h3>
              <p>{updater.update.notes || '本次更新暂无详细说明。'}</p>
            </section>
            {updater.installBlockedReason ? (
              <p className="update-dialog__warning">
                <TriangleAlert />
                {updater.installBlockedReason}
              </p>
            ) : null}
            <DialogFooter>
              <Button variant="outline" onClick={() => updater.setOpen(false)}>
                稍后更新
              </Button>
              <Button
                disabled={Boolean(updater.installBlockedReason)}
                onClick={() => void updater.installUpdate()}
              >
                <Download />
                立即更新
              </Button>
            </DialogFooter>
          </>
        ) : null}

        {updater.status === 'downloading' || updater.status === 'installing' ? (
          <div className="update-dialog__download">
            <LoaderCircle className="update-dialog__spinner" />
            <DialogHeader>
              <DialogTitle>
                {updater.status === 'installing' ? '正在安装更新' : '正在下载更新'}
              </DialogTitle>
              <DialogDescription>请保持应用开启，安装器启动后应用会自动退出。</DialogDescription>
            </DialogHeader>
            <div className="update-dialog__progress" role="progressbar" aria-valuenow={progress}>
              <span style={{ width: `${progress}%` }} />
            </div>
            <p>
              {formatBytes(updater.downloaded)}
              {updater.total ? ` / ${formatBytes(updater.total)}` : ''}
            </p>
          </div>
        ) : null}

        {updater.status === 'error' ? (
          <>
            <div className="update-dialog__state update-dialog__state--error">
              <TriangleAlert />
              <DialogHeader>
                <DialogTitle>更新未完成</DialogTitle>
                <DialogDescription>{updater.error}</DialogDescription>
              </DialogHeader>
            </div>
            <DialogFooter>
              <Button variant="outline" onClick={() => updater.setOpen(false)}>
                关闭
              </Button>
              <Button
                disabled={Boolean(updater.installBlockedReason)}
                onClick={() => void updater.retry()}
              >
                <RefreshCw />
                重试
              </Button>
            </DialogFooter>
          </>
        ) : null}
      </DialogContent>
    </Dialog>
  )
}
