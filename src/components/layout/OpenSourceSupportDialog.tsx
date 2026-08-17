import { HeartHandshake } from 'lucide-react'

import wechatDonationCode from '../../assets/wechat-donation.jpg'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger
} from '../ui/dialog'

export function OpenSourceSupportDialog() {
  return (
    <Dialog>
      <DialogTrigger asChild>
        <button
          className="window-title-bar__support-button"
          onDoubleClick={(event) => event.stopPropagation()}
          onMouseDown={(event) => event.stopPropagation()}
          title="查看开源声明与支持方式"
          type="button"
        >
          <HeartHandshake aria-hidden="true" />
          <span>开源免费 · 声明</span>
        </button>
      </DialogTrigger>
      <DialogContent
        className="buff-support-dialog sm:max-w-[720px]"
        onCloseAutoFocus={(event) => event.preventDefault()}
      >
        <DialogHeader className="buff-support-dialog__header">
          <div className="buff-support-dialog__icon" aria-hidden="true">
            <HeartHandshake />
          </div>
          <div>
            <DialogTitle>开源声明与支持开发</DialogTitle>
            <DialogDescription>本软件始终开源、完全免费。</DialogDescription>
          </div>
        </DialogHeader>
        <div className="buff-support-dialog__content">
          <div className="buff-support-dialog__copy">
            <section aria-labelledby="buff-open-source-notice">
              <span className="buff-support-dialog__kicker">OPEN SOURCE NOTICE</span>
              <h3 id="buff-open-source-notice">谨防第三方倒卖</h3>
              <p>本软件开源、完全免费。凡是对外收费售卖本软件均为第三方倒卖，请谨防被骗。</p>
            </section>
            <section aria-labelledby="buff-donation-notice">
              <span className="buff-support-dialog__kicker">VOLUNTARY SUPPORT</span>
              <h3 id="buff-donation-notice">打赏完全自愿</h3>
              <p>打赏纯属自愿，不强制，不提供特权，感谢支持开发者！</p>
            </section>
            <p className="buff-support-dialog__author">作者：401163814@qq.com</p>
          </div>
          <figure className="buff-donation-code">
            <img
              alt="开发者微信支付收款码"
              height="1128"
              loading="lazy"
              src={wechatDonationCode}
              width="828"
            />
            <figcaption>
              <strong>微信扫码支持</strong>
              <span>感谢你的认可与鼓励</span>
            </figcaption>
          </figure>
        </div>
      </DialogContent>
    </Dialog>
  )
}
