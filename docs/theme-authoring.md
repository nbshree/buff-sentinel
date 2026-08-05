# 职业主题包制作与接入规范

本文规定“shree盒子”内置职业主题的目录、代码、视觉素材和验收标准。目标是让后续职业主题只扩展主题层，不修改宏控制、方案、配置、流程或日志等业务组件。

## 1. 适用范围与固定边界

- 第一版只支持随应用编译发布的内置主题，不支持 ZIP、本地目录或网络主题导入。
- 主题是应用全局外观偏好，由 Rust 持久化；不属于宏方案，方案导入和导出不得包含主题。
- 禁止使用 `localStorage` 作为正式主题来源。
- 默认外观保持 `longyin` 且 `cleanMode: false`。新增主题不得暗中改变默认值。
- 业务组件不得判断具体职业或主题 ID。颜色、边框、阴影、字体、透明度和装饰差异必须通过语义 Token 与主题素材实现。
- 主题素材只负责氛围，不得包含按钮、表格、应用标题、状态、热键或其他功能文字。
- 主题层不得获得文件系统、Shell、网络或其他 Tauri 权限。

### 1.1 当前内置主题

主题卡片顺序由 `THEME_IDS` 决定，固定为
`default → longyin → chaoguang → xuehe → jiuling → suwen → shenxiang`。当前共七个内置主题，默认值仍固定为
`longyin`：

| 顺序 | ID          | 展示名称 | `profession` | 说明                 |
| ---: | ----------- | -------- | ------------ | -------------------- |
|    1 | `default`   | 默认简洁 | 省略         | 无职业素材的基础主题 |
|    2 | `longyin`   | 龙吟     | 龙吟         | 灰蓝墨色武侠主题     |
|    3 | `chaoguang` | 潮光     | 潮光         | 青白水光武侠主题     |
|    4 | `xuehe`     | 血河     | 血河         | 玄铁绛红枪阵主题     |
|    5 | `jiuling`   | 九灵     | 九灵         | 暮樱紫雾御灵主题     |
|    6 | `suwen`     | 素问     | 素问         | 月白青瓷疗愈主题     |
|    7 | `shenxiang` | 神相     | 神相         | 霁蓝雪岭清越琴心主题 |

血河主题以冷雾玄铁为基调，以克制绛红和旧铜点缀，营造冷峻枪阵氛围；避免大面积
正红、纯黑或亮金。

九灵主题以暮樱紫雾、玄木灵杖和幽蝶青辉为核心，角色与高对比环境细节集中在右侧；
左侧控制区和中部流程区保持浅色、低频、低对比。

素问主题以月白、青瓷绿、旧金莲饰和晨雾药庐为核心，丝带与人物收束在右侧；左侧控制区
和中部流程区保持安静、通透，不使用潮光的海蓝水光或九灵的紫雾灵蝶。

神相主题以霁蓝雪岭、墨黑漆琴、旧银琴徽和寒山琴台为核心，人物与古楼收束在右侧；
左侧控制区和中部流程区保持雾白、低频、低对比，不照搬龙吟的兵刃斜线或潮光的海境虹彩。

职业主题的 `name` 使用职业名本身，不添加“·意象”等自造后缀。`profession` 可以与
`name` 相同；主题选择器会在两者相同时隐藏职业徽标，避免出现“潮光 / 潮光”一类重复文案。

主题数据流固定为：

```text
Rust MacroState.appearance
  -> ThemeProvider 标准化并设置 data-theme / data-clean-mode
  -> ThemeDialog 本地即时预览
  -> 用户应用
  -> update_appearance
  -> Rust 清洗、保存、广播新状态
  -> ThemeProvider 使用持久化状态
```

## 2. 主题包目录

新职业使用稳定、全小写的 ASCII ID。推荐 `pinyin` 或 `kebab-case`，例如 `suimeng`、`blood-river`。ID 一旦发布即视为持久化协议，不得仅为改名而变更。

```text
src/themes/
├─ types.ts
├─ registry.ts
├─ themes.css
└─ <theme-id>/
   ├─ theme.ts
   ├─ theme.css                 # 新主题推荐独立维护，再由 themes.css 导入
   └─ assets/
      ├─ background.webp
      ├─ character.webp
      ├─ log-character.webp
      ├─ preview.webp
      ├─ paper-noise.webp
      ├─ corner-top-right.svg
      └─ corner-bottom-left.svg
```

规则：

- 文件和目录名只使用小写 ASCII、数字和连字符。
- `ThemeAssets` 的素材字段在运行时保持可选，以支持 `default` 主题和单素材故障降级；
  `default` 可以省略 `logCharacter`。但完整职业主题发布包必须提供第 6 节的七件套，
  包括 `log-character.webp`，不得以字段可选为由缺件发布或使用低质量占位图。
- `ThemeDefinition.preview` 当前是必填字符串。职业主题必须提供正式预览图；无素材的基础主题可以使用空字符串触发现有图标回退。
- 主题专属 CSS 只允许定义 Token、装饰层位置和主题选择卡片的无图回退色，不得覆盖业务组件内部结构。
- 当前 `default` 和 `longyin` 的 Token 位于 `src/themes/themes.css`，`chaoguang`、
  `xuehe`、`jiuling`、`suwen` 和 `shenxiang` 分别使用独立的主题 CSS，并在中央样式文件顶部导入。
  后续主题沿用独立文件方式；不要把职业规则散落到 `src/assets/main.css`。

## 3. ThemeDefinition 约定

当前类型定义位于 `src/themes/types.ts`：

```ts
type ThemeAssets = {
  background?: string
  character?: string
  logCharacter?: string
  texture?: string
  cornerTopRight?: string
  cornerBottomLeft?: string
}

type ThemeDefinition = {
  id: ThemeId
  name: string
  profession?: string
  description: string
  preview: string
  assets: ThemeAssets
}
```

字段要求：

| 字段          | 规范                                                                    |
| ------------- | ----------------------------------------------------------------------- |
| `id`          | 与目录名、`THEME_IDS`、注册表键和 Rust 白名单完全一致；发布后保持稳定。 |
| `name`        | 选择器中展示的短名称。职业主题直接使用职业名，例如“龙吟”“潮光”。        |
| `profession`  | 结构化职业名；基础主题可省略。与 `name` 相同时选择器不重复显示徽标。    |
| `description` | 一句话描述色彩与氛围，不描述不存在的功能，建议不超过 40 个中文字符。    |
| `preview`     | 480×300 WebP 的 Vite 资源 URL，不得直接写运行时绝对路径。               |
| `assets`      | 只声明实际存在的分层素材；每个字段独立加载和独立降级。                  |

`theme.ts` 推荐模板：

```ts
import type { ThemeDefinition } from '../types'

export const suimengThemeAssetPaths = {
  background: './assets/background.webp',
  character: './assets/character.webp',
  logCharacter: './assets/log-character.webp',
  preview: './assets/preview.webp',
  texture: './assets/paper-noise.webp',
  cornerTopRight: './assets/corner-top-right.svg',
  cornerBottomLeft: './assets/corner-bottom-left.svg'
} as const

export const suimengTheme: ThemeDefinition = {
  id: 'suimeng',
  name: '碎梦',
  profession: '碎梦',
  description: '用一句话说明主题的主色、材质和视觉气质。',
  preview: new URL('./assets/preview.webp', import.meta.url).href,
  assets: {
    background: new URL('./assets/background.webp', import.meta.url).href,
    character: new URL('./assets/character.webp', import.meta.url).href,
    logCharacter: new URL('./assets/log-character.webp', import.meta.url).href,
    texture: new URL('./assets/paper-noise.webp', import.meta.url).href,
    cornerTopRight: new URL('./assets/corner-top-right.svg', import.meta.url).href,
    cornerBottomLeft: new URL('./assets/corner-bottom-left.svg', import.meta.url).href
  }
}
```

必须使用 `new URL('./assets/...', import.meta.url).href`，以便 Vite 在开发、生产构建和 Tauri 安装包中正确收集并重写资源路径。不要使用 `public` 目录绝对路径、磁盘绝对路径或运行时字符串拼接。

## 4. 注册步骤

新增主题是一个跨前后端的注册事务。以下步骤必须在同一个提交完成，否则可能出现“选择器能看到，但保存或重启后回退龙吟”的不一致状态。

1. 在 `src/themes/types.ts` 的 `THEME_IDS` 中追加 ID。该数组同时决定 `ThemeId` 联合类型和主题卡片顺序。
2. 在 `src/themes/registry.ts` 导入主题定义，并向 `themeRegistry` 增加同名键。
3. 在 `src/themes/themes.css` 导入主题专属 CSS，或在该文件中增加唯一的 `:root[data-theme='<theme-id>']` Token 块。
4. 在 `src-tauri/src/model.rs` 的 `sanitize_theme_id` 中加入同一 ID。不要改变未知 ID 回退 `DEFAULT_THEME_ID` 的行为。
5. 为前端注册表和 Rust 清洗逻辑各增加一个该 ID 的测试用例。
6. 如果导出素材路径常量供测试使用，可从 `src/themes/index.ts` 导出；业务组件不得依赖该常量。

新增普通职业主题不应修改 `App.tsx`、`ThemeProvider.tsx`、`ThemeDialog.tsx`、`ThemeBackground.tsx` 或任何业务面板。若接入必须修改这些文件，先检查是否把职业特例错误放进了通用层。

## 5. 语义 Token 合同

所有功能界面只消费语义 Token。主题 CSS 可以改变 Token 的值，但不得让业务组件使用主题专属颜色名或原始十六进制色值。

### 5.1 必须检查的 Token 组

| 用途           | Token                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 应用底色       | `--app-background`、`--app-background-elevated`                                                                                                                                                                                                                                                                                                                                                                                                            |
| 面板与控件表面 | `--surface-panel`、`--surface-panel-strong`、`--surface-muted`、`--surface-input`、`--surface-input-disabled`、`--surface-hover`                                                                                                                                                                                                                                                                                                                           |
| 文字           | `--text-primary`、`--text-secondary`、`--text-muted`、`--text-on-primary`                                                                                                                                                                                                                                                                                                                                                                                  |
| 边界           | `--border-default`、`--border-strong`                                                                                                                                                                                                                                                                                                                                                                                                                      |
| 主操作         | `--color-primary`、`--color-primary-hover`、`--color-primary-active`、`--focus-ring`                                                                                                                                                                                                                                                                                                                                                                       |
| 语义状态       | `--color-accent`、`--color-danger`、`--color-danger-hover`、`--color-danger-soft`、`--color-success`                                                                                                                                                                                                                                                                                                                                                       |
| 日志与弹窗     | `--log-background`、`--log-text`、`--scrim`                                                                                                                                                                                                                                                                                                                                                                                                                |
| 层级效果       | `--shadow-panel`、`--shadow-dialog`                                                                                                                                                                                                                                                                                                                                                                                                                        |
| 装饰透明度     | `--theme-background-opacity`、`--theme-texture-opacity`、`--theme-character-opacity`、`--theme-corner-opacity`                                                                                                                                                                                                                                                                                                                                             |
| 流程玻璃层     | `--surface-workspace-panel`、`--flow-header-surface`、`--flow-muted-surface`、`--flow-row-surface`、`--flow-row-hover-surface`、`--flow-row-active-surface`、`--flow-control-surface`、`--flow-control-disabled-surface`、`--flow-control-hover-surface`、`--flow-primary-surface`、`--flow-primary-hover-surface`、`--flow-key-editor-surface`、`--flow-badge-surface`、`--flow-text-secondary`、`--flow-text-muted`、`--workspace-panel-backdrop-filter` |

`--radius-panel`、`--radius-control`、`--motion-fast`、`--motion-normal`、`--ease-standard`、`--ui-font` 和 `--heading-font` 是应用级一致性 Token。职业主题原则上继承，不单独改变。控件继续使用系统中文字体；主题不提供职业题字素材，也不随主题打包装饰字体。

### 5.2 CSS 模板

```css
:root[data-theme='suimeng'] {
  color-scheme: light;
  --app-background: #e8edf2;
  --app-background-elevated: #f3f6f8;
  --surface-panel: rgb(248 250 252 / 90%);
  --surface-panel-strong: rgb(252 253 254 / 97%);
  --surface-muted: rgb(226 232 238 / 88%);
  --surface-input: rgb(252 253 254 / 94%);
  --surface-input-disabled: rgb(216 223 230 / 92%);
  --surface-hover: rgb(216 225 233 / 90%);
  --text-primary: #203047;
  --text-secondary: #46566b;
  --text-muted: #637388;
  --text-on-primary: #ffffff;
  --border-default: #afbbc8;
  --border-strong: #7f90a3;
  --color-primary: #304f78;
  --color-primary-hover: #274265;
  --color-primary-active: #203752;
  --color-accent: #a88352;
  --color-danger: #9e3f49;
  --color-danger-hover: #85323b;
  --color-danger-soft: #f7e9eb;
  --color-success: #356b57;
  --focus-ring: rgb(48 79 120 / 34%);
  --shadow-panel: 0 10px 30px rgb(31 49 72 / 13%);
  --shadow-dialog: 0 30px 90px rgb(20 33 51 / 38%);
  --log-background: rgb(21 34 53 / 96%);
  --log-text: #e4ebf2;
  --scrim: rgb(15 26 41 / 58%);
  --theme-background-opacity: 0.88;
  --theme-texture-opacity: 0.14;
  --theme-character-opacity: 0.9;
  --theme-corner-opacity: 0.58;
}
```

示例色值只展示结构，不是新职业可直接复用的调色板。发布前必须逐对验证正文、按钮、错误信息和日志的对比度。

### 5.3 Token 使用禁区

- 不在 React 中写 `theme.id === 'xxx'` 来改变布局、文案或行为。
- 不在业务组件 CSS 中增加 `[data-theme='xxx'] .flow-panel` 一类职业覆盖。
- 不把功能状态编码进主题色；运行、警告、错误和成功始终使用现有语义状态。
- 不用颜色作为唯一的选中、错误或运行状态提示，必须同时保留文字、图标或结构变化。
- 不为单个职业覆盖控件高度、流程列宽、日志高度或标题栏尺寸。
- 主题切换动效使用共享时长与缓动 Token。可感知的颜色和透明度过渡控制在 180–250ms，并继续支持 `prefers-reduced-motion`。

## 6. 素材规格与预算

单个完整职业主题七件套总计不得超过 `2,000,000 B`。以下是发布上限，不是建议尽量用满的额度。

| 文件                     | 像素与格式     |   上限 | Alpha | 用途                       |
| ------------------------ | -------------- | -----: | ----- | -------------------------- |
| `background.webp`        | 1920×1280 WebP | 500 KB | 否    | 环境、山水、光影等全窗底图 |
| `character.webp`         | 1200×1280 WebP | 1.2 MB | 是    | 右下角角色独立层           |
| `log-character.webp`     | 384×384 WebP   | 250 KB | 是    | 日志左侧 Q 版职业人物      |
| `preview.webp`           | 480×300 WebP   | 120 KB | 否    | 16:10 主题卡片预览         |
| `paper-noise.webp`       | 512×512 WebP   | 100 KB | 否    | 全窗拉伸使用的低对比纹理   |
| `corner-top-right.svg`   | SVG            |  30 KB | 是    | 右上边角装饰               |
| `corner-bottom-left.svg` | SVG            |  30 KB | 是    | 左下边角装饰               |

通用输出要求：

- 七件套总体积必须不超过 `2,000,000 B`，`log-character.webp` 必须计入总预算。
- 位图统一使用 sRGB，去除 EXIF、缩略图、定位和其他无关元数据。
- 透明 WebP 不得残留白边、黑边、绿幕边或半透明脏底；在深色和浅色底上分别检查轮廓。
- `log-character.webp` 必须是 384×384 sRGBA WebP，包含真实透明像素而不是全不透明 Alpha 通道，
  四个画布角必须完全透明。
- 背景、全屏角色、日志人物、纹理和边角必须完全分层，任何单层隐藏后都不能留下重复残影。
- SVG 必须提供 `viewBox`，优先使用路径和基础图形；禁止脚本、外链图片、外链字体、滤镜堆栈和不可控的嵌入资源。
- 不在素材内烘焙功能 UI、应用名称、按钮、表格、状态文字或热键。
- 所有主题素材都不得烘焙职业题字或其他装饰文字；职业名称只通过界面文本展示。
- 装饰不得成为理解功能的必要信息；关闭全部素材时，应用仍应完整可操作。

### 6.1 血河主题素材基线

- 参考图只用于服装、兵器和气质参考，不直接裁切。全部素材不得包含职业题字、Logo、
  红色文字栏、`vertical-mark`、带字旗帜或血液飞溅。
- `background.webp` 使用冷灰晨雾中的河谷古战场，枪阵、城塞轮廓和少量绛红布带集中
  在右侧；左侧控制区和中部流程区保持低频、低对比，不包含人物。
- `character.webp` 为透明背景男性枪客，三分之二至近全身构图，使用黑铁甲、旧铜纹饰、
  深绛战袍和高马尾红束带。人物朝左或左上并锚定右下，长枪沿右肩或画布右侧竖置，
  不斜穿流程区域。
- 角色先以纯绿幕重新生成，再使用本地 chroma-key 工具抠除。必须在白色、玄铁灰和酒红
  底色上以 200% 检查发丝、枪杆、甲片和衣摆；一次去边仍有绿边时使用
  `edge-contract 1` 重试，仍不合格则停止，不切换到需要 API Key 的透明图工具。
- `paper-noise.webp` 使用冷烟尘、旧宣纸和极细锻铁颗粒，不包含血迹、裂纹或明显方向性
  斑块。右上边角使用枪锋、甲片铆钉和细绛红缎带；左下边角使用抽象河流弧线、甲片
  几何和少量旧铜节点。
- `preview.webp` 由最终背景、角色和边角离线合成，左侧保留雾白空间，右侧展示左望枪客；
  不烘焙 UI、名称或 Logo。
- 七件套继续遵守本节尺寸和单文件上限，主题总素材不超过 2,000,000 B；不提交绿幕源图、失败
  候选或临时合成文件。

## 7. 构图、安全区与锚点

背景以 `object-fit: cover; object-position: center` 铺满窗口；角色以右下角为锚点；其他素材均位于功能面板之后。制作时必须考虑窗口比例变化和 CSS 负偏移造成的裁切。

### 7.1 背景

- 画布为 1920×1280。外侧 10% 视为潜在裁切区，不放不可缺失的主体、徽记或高对比细节。
- 左侧约 32% 是控制栏阅读区；中部约 34%–78% 是流程和日志阅读区。这些区域保持低频、低对比，不放人物面部、武器交叉点或强烈光斑。
- 视觉焦点优先放在右侧约 30%，并与角色层形成一致光向。
- 背景不能承担角色轮廓，也不得烘焙职业题字或其他装饰文字；角色必须放到独立层。

### 7.2 角色

- 角色画布固定 1200×1280，背景完全透明，人物朝向内容区，即视觉方向优先朝左或左上。
- 右侧 16% 和底部 8% 视为可裁切区，不能放脸、手、武器握点、职业徽记等关键内容。
- 左侧至少保留约 15% 的透明或渐隐缓冲，避免角色形成硬直边压住正文。
- 脚、衣摆或特效可以延伸到右下锚点，但关键轮廓必须在安全区内闭合。
- 不在角色图中加入投向功能面板的高不透明纯黑阴影；角色存在感由主题透明度 Token 控制。

### 7.3 纹理与边角

- `paper-noise.webp` 当前由 `ThemeBackground` 作为全窗 `<img>` 渲染，使用 `width/height: 100%` 与 `object-fit: cover` 拉伸和裁切，并不是 CSS 平铺纹理。制作时应在横向、纵向拉伸及不同窗口比例下检查颗粒尺度、亮斑和边缘，不能只验证四边无缝。
- 纹理混合模式可以由主题样式指定；默认主题体系使用 `multiply`，潮光使用 `soft-light`。无论使用哪种模式，低透明度下都不能造成正文脏污、色偏或闪烁。
- 边角 SVG 的主要线条保持在各自 `viewBox` 内；贴边部分允许自然裁切，中心方向留出渐隐空间。
- 不依靠边角素材遮盖背景或角色的接缝。

### 7.4 日志人物

- `log-character.webp` 固定为 384×384，使用独立 Q 版构图，保留完整方形画布、柔光和 Alpha；
  不从 `character.webp` 直接裁切、不烘焙文字或功能 UI。
- 人物底部对齐并在日志面板左侧的独立透明位置内居中，主要轮廓不贴死四边，缩小到
  112–140 CSS px 的占位宽度时
  仍能辨识职业特征。
- 该素材不属于全屏 `ThemeBackground` 图层，不应改变背景的安全区、锚点或五层测试合同。

### 7.5 预览图

- 预览固定 480×300，保留 16:10 比例，四周至少 12 px 视觉安全区。
- 展示背景、角色和装饰的最终关系，但不模拟或烘焙真实按钮、表格和功能文字。
- 缩小到主题卡片后仍应能通过主色、人物剪影和构图区分主题。

## 8. 装饰层与加载降级

通用层级顺序已经由 `ThemeBackground` 固定：

```text
纯色 app background
  -> background
  -> texture
  -> character
  -> cornerTopRight / cornerBottomLeft
  -> 功能内容和面板
```

- `logCharacter` 不属于上述五个全屏素材层；它由通用日志人物组件根据
  `activeAppearance` 在日志面板左侧的独立透明位置渲染，主题预览时应立即跟随切换。
  人物与日志 `<section>` 是兄弟节点；人物位不得使用面板底色、边框、阴影或圆角。
- 所有装饰图片使用空 `alt`、`aria-hidden="true"`、`draggable={false}` 和 `pointer-events: none`，不进入键盘焦点顺序。
- `ThemeProvider` 会预加载每个声明的资源，包括 `logCharacter`，并维护
  `idle/loading/loaded/error` 状态。
- `ThemeBackground` 会独立隐藏缺失、预加载失败或 `<img>` 二次加载失败的五个全屏层。
  日志人物组件对 `logCharacter` 采用同等的独立降级：字段缺失或加载失败时收起整个独立
  人物位，日志恢复全宽；切换到有效素材的主题后可再次显示。主题 CSS 不得假设任意素材
  一定存在。
- 预览图失败时选择器使用现有图标回退；主题名称、职业和描述必须仍可识别该主题。
- 不要把可选素材改写为 CSS `background-image: url(...)`，否则会绕过现有加载状态和单层降级机制。
- 任何素材失败时，纯色背景、面板、控件和正文都必须保持足够对比度；禁止依赖背景图中的暗区或亮区保证可读性。
- 运行时类型允许基础主题或故障降级省略可选素材；完整职业主题仍必须提供七件套。
  不得通过透明空文件伪造缺失的纹理、边角或日志人物。

## 9. 响应式与纯净模式

应用最小窗口为 1080×700，默认窗口为 1280×820。主题包必须服从通用布局，不能通过主题 CSS 改变工作区尺寸。

主题选择器继续由通用 `themes.map()` 渲染，不增加职业专属判断。弹窗宽度固定为
`min(920px, calc(100vw - 32px))`；大于 880px 时每行四列，超出后自然换行；宽度不超过
880px 时两列，不超过 520px 时单列。

| 窗口宽度      | 通用行为                | 主题素材要求                                       |
| ------------- | ----------------------- | -------------------------------------------------- |
| `>=1320px`    | 左栏 304 px，完整工作区 | 可显示完整角色和边角装饰。                         |
| `1180–1319px` | 左栏 304 px，角色缩小   | 保持人物关键区域可见，不把高对比主体移入表格中心。 |
| `1080–1179px` | 左栏 288 px，间距收紧   | 角色透明度降低并右移，不能出现全局横向滚动。       |

补充要求：

- 断点按 CSS 像素计算，不按素材像素或 DPI 缩放比计算。
- 主题不得为流程表格增加固定最小宽度，不得改变 `304/288 px` 左栏、`34–36 px` 控件和 `40–44 px` 流程行规范。
- 角色和装饰可以被面板覆盖，但不得遮挡点击、拖动、滚动或窗口缩放区域。
- 100%、125%、150% DPI 和混合 DPI 下都不得出现全局横向滚动。

纯净模式由根节点 `data-clean-mode='true'` 控制：

- 保留主题的主色、文字、边框、状态色和日志配色。
- 隐藏背景图、角色、纹理、全部边角装饰和独立日志人物位，日志恢复全宽。
- 使用纯色应用背景和高不透明度面板、输入表面。
- 主题不得使用更高优先级或 `!important` 恢复装饰透明度。
- 不要在业务面板伪元素中绘制职业纹样；这类内容不会被通用纯净模式自动识别。

## 10. 可访问性与交互质量

- 正常字号正文与其实际表面背景的对比度至少为 4.5:1；大字号文字和非文本控件边界至少为 3:1。
- 至少验证以下组合：主文字/面板、次级文字/面板、文字/输入框、按钮文字/主按钮三个状态、错误文字/错误底色、日志文字/日志背景。
- 焦点环在所有面板和控件表面上清晰可见，不得通过主题移除 `:focus-visible`。
- 主题卡片的选中状态不能只靠颜色，需保留单选语义、勾选图标和主题名称。
- 装饰图保持屏幕阅读器不可见；主题预览图使用“主题名称 + 主题预览”的有效替代文本。
- 不改变原生 `dialog`、单选框和复选框的键盘语义。Tab、方向键、Space、Enter 和 Esc 必须按现有行为工作。
- 动效只用于颜色、透明度和轻微变换，不通过宽高变化引发布局抖动；`prefers-reduced-motion: reduce` 下接近即时完成。
- 不打包远程字体或整套装饰字体。中文控件必须在 Windows 系统字体回退链下可读。

## 11. 新增职业主题的标准流程

1. 确定稳定 ID、展示名称、职业名、描述、主色和视觉意象。
2. 按本规范建立 `<theme-id>/theme.ts`、`theme.css` 和 `assets/`。
3. 先输出背景、全屏角色、日志 Q 版人物、纹理和边角独立素材，再制作最终预览图；
   不要从带 UI 的整张效果图直接切块发布。
4. 在浅色和深色临时底上检查两张人物图的透明边缘和四角，在 384×384 下检查日志人物构图，
   在 480×300 下检查预览辨识度。
5. 完成 ThemeDefinition，所有资源使用 `new URL(..., import.meta.url).href`。
6. 完成 Token 映射和必要的装饰层位置覆盖；不修改业务组件。
7. 一次性同步 TypeScript ID、注册表、CSS 导入和 Rust `sanitize_theme_id`。
8. 增加注册、标准化、持久化白名单测试。
9. 执行自动化验证和三档窗口手工验收。
10. 检查 Git 变更，确保没有提交临时源图、生成目录、`dist/` 或 `src-tauri/target/`。

## 12. 验收清单

### 12.1 代码与数据

- [ ] ID 为稳定的小写 ASCII，并在 TypeScript 与 Rust 中完全一致。
- [ ] `ThemeDefinition` 字段完整，所有资源使用 `new URL(..., import.meta.url).href`。
- [ ] 主题只通过 Token 和通用装饰层实现，业务组件不存在主题 ID 判断。
- [ ] 未改变默认 `longyin`、未知主题回退和旧配置迁移行为。
- [ ] 外观仍是全局设置，不进入方案导入和导出。
- [ ] 主题可在录制或运行期间切换，且不修改方案更新时间或宏运行状态。

### 12.2 素材

- [ ] 每张位图像素尺寸、格式和单文件体积符合表格上限。
- [ ] 完整职业主题包含七件套；单主题总素材（含日志人物）不超过 2,000,000 B。
- [ ] 角色拥有干净 Alpha，无白边、黑边、绿边和脏底。
- [ ] `log-character.webp` 为 384×384 sRGBA WebP，不超过 250,000 B，含真实透明且四角完全透明。
- [ ] 纹理四边无缝，边角 SVG 无脚本、外链、字体和嵌入位图。
- [ ] 背景、全屏角色、日志人物、纹理和边角完全分层，无重复内容。
- [ ] 素材不含按钮、表格、应用标题或其他功能文字。
- [ ] 构图满足安全区，角色朝向内容区并锚定右下。

### 12.3 视觉与交互

- [ ] 1080×700、1280×820、最大化下均无全局横向滚动。
- [ ] 100%、125%、150% DPI 和混合 DPI 下没有遮挡、模糊边缘或错误裁切。
- [ ] 0、3、20 条流程数据下正文和控件可读，角色不抢占操作层级。
- [ ] 主题选择即时预览；取消、Esc 和点击遮罩恢复原主题。
- [ ] 应用后可持久化，重启恢复；保存失败回滚并显示明确错误。
- [ ] 纯净模式隐藏全部装饰但保留主题配色和高对比面板。
- [ ] 任意单层素材加载失败时只隐藏故障层，功能界面仍可操作。
- [ ] 日志人物缺失、加载失败或处于纯净模式时，独立人物位收起且日志恢复全宽。
- [ ] 键盘焦点可见，选择状态不依赖颜色，正文对比度达到 4.5:1。
- [ ] 减少动态效果系统设置下没有持续或明显位移动画。

### 12.4 自动化验证

```text
pnpm test
pnpm typecheck
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
pnpm tauri:build
```

提交前还应执行 `git diff --check`，并确认安装包内能够加载全部主题素材。素材尺寸可使用 ImageMagick 的 `magick identify` 检查，体积可使用 PowerShell 的 `Get-ChildItem` 汇总；这些检查不替代实际 Tauri 窗口和 DPI 验收。
