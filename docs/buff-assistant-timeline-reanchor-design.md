# Buff 助手时间轴实际触发重锚设计与交接

更新时间：2026-08-03

## 背景

录屏显示金周天实际触发间隔约为 20.2～20.3 秒，而原实现始终沿旧预测截止点按
固定 20 秒推进。即使迟到的触发仍处于宽限期内，下一轮也不会按实际出现时间校准，
因此预测会逐轮提前：约 0.2 秒、0.5 秒、0.8 秒，并最终超过宽限期。

另一个体验问题是：预测截止点已经到达、后台仍在宽限期内等待连续帧确认时，悬浮窗
会一直显示 `0.0 秒`，无法区分正常等待和运行卡住。

## 设计目标

- 首次及后续真实触发都以连续匹配序列的第一帧时间为锚点。
- 后续截止点统一为“实际触发锚点 + 配置周期”，消除累计漂移。
- 宽限期只控制迟到触发是否仍可接受，不计入周期。
- 截止点到达但尚未确认时，进入明确的“等待金周天确认”状态。
- 保持默认周期 20 秒，不迁移已有配置，不引入自动学习周期。

## 目标运行逻辑

1. 首次监控仍需先确认图标消失，再接受图标连续出现。
2. 连续命中达到确认帧数后，使用该序列第一帧的 `detected_at` 建立时间轴。
3. 到达预测截止点但图标尚未确认时，进入 `Confirming` 阶段并开始消耗宽限期。
4. 宽限期内确认成功时，使用本次 `detected_at` 重新锚定下一轮。
5. `detected_at` 缺失或晚于确认时间时，回退到当前确认时间。
6. 宽限期结束仍未确认时，清空时间轴并回到未武装的等待状态。

## 已完成

### Rust 时间轴

- `TimelinePhase` 已增加 `Confirming`。
- `TimelineAction` 已增加一次性的 `ConfirmationPending`。
- 截止点到达但尚未确认时会进入 `Confirming`，后续帧不会重复发送等待动作。
- 宽限期内确认成功时已改用 `valid_detection_time(now, detected_at)` 重锚，不再使用
  旧 `expected_at`。
- 超过宽限期后的重置逻辑保持不变。
- 已更新或新增时间轴测试代码，覆盖：
  - 首次命中帧锚定；
  - 宽限期边界；
  - 无效未来时间回退；
  - 20.2、20.3、20.3 秒连续触发不累计漂移。

注意：上述测试代码已经写入，但本次没有运行。

### Tauri 状态与事件

- `BuffAssistantActivity` 已增加 `Confirming`。
- `BuffOverlayMode` 已增加 `Confirming`。
- 时间轴进入确认期时会发出新的运行状态和悬浮窗状态。
- 确认期悬浮窗消息为“等待金周天确认”，且不携带倒计时截止时间。
- 真实触发日志已改为“已按实际触发时间校准倒计时”。
- 确认期开始时会记录“正在宽限期内等待金周天确认”。

### React 类型与显示

- TypeScript 的活动状态和悬浮窗模式联合类型已同步增加 `confirming`。
- Buff 助手主页面已为 `confirming` 增加“等待触发确认”文案。
- 悬浮窗现有渲染分支会在 `confirming` 模式下只显示消息，不再显示 `0.0 秒`。
- 已增加确认期悬浮窗的呼吸动画样式。

## 未完成

- 主页面状态徽标 CSS 尚未把 `confirming` 纳入金色运行状态，目前会显示默认灰色。
- 尚未新增前端组件测试，需要覆盖：
  - `confirming` 模式显示“等待金周天确认”；
  - `confirming` 模式不显示倒计时数字；
  - 收到新的 `countdown` 事件后按新的 `expectedAtUnixMs` 恢复倒计时。
- 尚未运行 Rust 时间轴测试，也未确认新增测试能通过编译。
- 尚未运行前端测试、TypeScript 检查或生产构建。
- 尚未运行 Rust `fmt/check/clippy`。
- 尚未执行 Tauri 安装包构建。
- 尚未使用游戏或录屏场景进行手工复测。

## 下次实施顺序

1. 在 `BuffAssistantPage.css` 中把 `confirming` 加入 `waiting/tracking/prewarning` 的
   运行状态选择器，并为确认期保留清晰但不过度强烈的提示效果。
2. 为 `BuffOverlayApp` 增加组件测试，模拟 `buff-overlay-state` 事件完成确认期与新倒计时
   的状态切换验证。
3. 先单独运行 Buff 时间轴 Rust 测试，修正可能存在的旧断言或边界问题。
4. 串行执行 `pnpm test`、`pnpm typecheck`、`pnpm build`。
5. 串行执行 Rust `fmt --check`、`cargo check`、`cargo clippy -- -D warnings` 和测试。
6. 执行 `pnpm tauri:build`。
7. 使用周期 20 秒、宽限期 2000 毫秒手工复测：
   - 迟到 0.2～0.8 秒的触发应从实际出现时间重新起算；
   - 多轮运行后预测误差不应逐轮累积；
   - 确认期间悬浮窗应显示“等待金周天确认”；
   - 超过 2 秒仍未确认时应重置时间轴。

## 当前涉及文件

- `src-tauri/src/buff_assistant/timeline.rs`
- `src-tauri/src/buff_assistant/model.rs`
- `src-tauri/src/buff_assistant/mod.rs`
- `src/lib/macro-api.ts`
- `src/features/buff-assistant/BuffAssistantPage.tsx`
- `src/features/buff-assistant/BuffOverlayApp.css`

