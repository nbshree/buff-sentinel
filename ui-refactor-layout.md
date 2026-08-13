# UI Refactoring Plan: Sidebar + Dashboard Layout with Premium Gamer Dark/Gold Theme

This document outlines the proposed layout structure, style variables, and code adjustments to refactor the UI of the Buff Sentinel desktop application main page (`BuffAssistantPage.tsx` and `BuffAssistantPage.css`).

## 1. Objective
Refactor the main interface of the application to improve usability, visual appeal, and information hierarchy.
- **Layout:** Shift from a stacked vertical layout of cards to a modern split-panel layout:
  - **Left Sidebar:** Window selection, capture preview, and shared Buff search region selection.
  - **Right Dashboard Content:**
    - **Header:** Activity status bar and monitoring/floating-window controls.
    - **Main Body:** A grid list of listener cards (icon, name, state, threshold confidence, action buttons).
    - **Footer / Collapsible Panel:** Execution logs with clean layout and monospaced typography.
- **Visuals:** Implement a premium gaming assistant theme (Gamer Dark/Gold):
  - Deep dark background with subtle radial gradient.
  - Glassmorphic panels with gold borders (`rgb(212 163 89 / 20%)`) and gold glow.
  - Consistent layout heights, hover feedback transitions, clear status badges, and refined typography.

---

## 2. Key Files & Context

- `src/assets/main.css`: Global styles, CSS variables, body background, and Scrollbar rules. We will update the global color tokens to support a sleek gamer dark/gold background by default.
- `src/features/buff-assistant/BuffAssistantPage.tsx`: The main page component rendering the controls, window capture, listener list, logs, and dialogs.
- `src/features/buff-assistant/BuffAssistantPage.css`: Component-specific styles. We will restructure the grid layout classes, convert components into the sidebar/dashboard structure, and restyle listeners, controls, and logs.
- `src/components/layout/WindowTitleBar.tsx` & `src/components/layout/WindowTitleBar.css`: The top title bar. We will style it to match the new dark gamer theme.

---

## 3. Proposed Changes

### A. Theme Variables (`src/assets/main.css`)
We will redefine the default theme variables in `src/assets/main.css` to present a dark gamer aesthetic directly, while keeping it responsive and clean:

```css
:root {
  color-scheme: dark;
  --app-background: #0d0f12; /* Deep dark slate */
  --surface-panel: #161a22; /* Card panels */
  --surface-panel-strong: #1b202a; /* Popovers/Dialogs */
  --surface-muted: #1c2330; /* Subdued inputs/containers */
  --surface-hover: #263043; /* Hover highlights */
  --surface-input: #12161f; /* Input fields */
  
  --text-primary: #f1f5f9; /* Near white */
  --text-secondary: #94a3b8; /* Muted slate */
  --text-muted: #64748b; /* Slate-500 */
  --text-on-primary: #0f172a; /* Dark text on gold */
  
  --border-default: #262e3d; /* Subdued borders */
  --border-strong: #3b485f; /* Highlighted borders */
  
  --color-primary: #d4a359; /* Gold accent */
  --color-primary-hover: #e5b873; /* Golden yellow hover */
  --color-danger: #ef4444; /* Red danger */
  --color-danger-hover: #f87171;
  --color-warning: #f59e0b; /* Amber warning */
  
  /* Layout specific */
  --sidebar-width: 320px;
}
```

We will also update the background gradient of the body to feel more immersive:
```css
body {
  margin: 0;
  overflow: hidden;
  background: 
    radial-gradient(circle at 0% 0%, rgb(212 163 89 / 8%) 0%, transparent 40%),
    radial-gradient(circle at 100% 100%, rgb(30 41 59 / 40%) 0%, transparent 60%),
    var(--app-background);
  color: var(--text-primary);
  /* ... */
}
```

### B. UI Structure Refactoring (`src/features/buff-assistant/BuffAssistantPage.tsx`)
Currently, the page is rendered as a list of `section` cards with class `.buff-assistant-page`.
We will change this structure to:
```tsx
<div className="buff-assistant-page">
  {/* Left Sidebar */}
  <aside className="buff-sidebar">
    {/* Target Game Window selection */}
    {/* Capture Preview & Buff search region selector */}
  </aside>

  {/* Right Main Content */}
  <main className="buff-dashboard">
    {/* Top Header / Control Toolbar */}
    {/* Listeners Grid List */}
    {/* Bottom Logs Panel */}
  </main>
  
  {/* Dialogs remain identical to maintain logic */}
</div>
```

### C. Style Refactoring (`src/features/buff-assistant/BuffAssistantPage.css`)

1. **Main Layout Grid:**
   Use standard CSS flex or grid to split the page into sidebar and main dashboard:
   ```css
   .buff-assistant-page {
     display: flex;
     width: 100%;
     height: 100%;
     gap: 16px;
     padding: 16px;
     overflow: hidden; /* Avoid main viewport scrolls, scroll inside sections */
   }
   
   .buff-sidebar {
     width: var(--sidebar-width);
     display: flex;
     flex-direction: column;
     gap: 16px;
     flex-shrink: 0;
     overflow-y: auto;
     padding-right: 4px; /* Scrollbar spacing */
   }
   
   .buff-dashboard {
     flex: 1;
     display: flex;
     flex-direction: column;
     gap: 16px;
     min-width: 0;
     height: 100%;
   }
   ```

2. **Refined Cards & Panels:**
   Cards should use high-quality borders, border-radius, and glassmorphism styling:
   ```css
   .buff-card,
   .buff-execution-log,
   .buff-sidebar-section {
     border: 1px solid var(--border-default);
     border-radius: 12px;
     background: rgba(22, 26, 34, 0.75);
     backdrop-filter: blur(12px);
     box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
     transition: border-color var(--motion-fast) var(--ease-standard), box-shadow var(--motion-fast) var(--ease-standard);
   }
   ```
   Provide interactive focus/hover effects to cards to make them feel responsive.

3. **Window Selector & Capture in Sidebar:**
   - Compress the selection select box and capture preview button.
   - Refactor `RegionSelector` to look clean inside a narrower column (restrict maximum height or scale appropriately).

4. **Monitoring Action Toolbar:**
   - Arrange control actions at the top of the dashboard.
   - Show an elegant status badge indicating whether it is monitoring, waiting, or stopped.

5. **Listener List Grid:**
   - Convert the listener list into an active grid with clean cards instead of a vertical list of text blocks.
   - Each listener card should display:
     - Header: Checked state (checkbox), listener name.
     - Body: A small thumbnail of the cropped template image (if configured), status labels (Waiting, Tracking, Stopped), matching confidence level.
     - Actions: Play/Stop test, Edit, Delete.
   - Make configuration states visible and visually elegant.

6. **Execution Logs Panel:**
   - Place it as a fixed-height collapsible drawer or box at the bottom of the dashboard.
   - Enhance readability of logs with modern monospace typography, colors highlighting different log severities (e.g., gold/yellow for updates, red for errors, slate for standard checks).

---

## 4. Verification & Testing

1. **Visual inspection:**
   Run the dev server and visually inspect the interface in Tauri/browser. Ensure that alignment is correct, layout wraps properly on resize, dialog overlays display centered, and inputs/buttons react correctly to user events.
2. **Component unit tests:**
   Run `pnpm test` to verify that `BuffAssistantPage.test.tsx` and related tests still pass. Any changes to button text, roles, or class selectors must be updated accordingly in the test files to prevent breakage.

---

## 5. Migration & Backwards Compatibility
Since we are only refactoring CSS classes and HTML layout elements in the React codebase:
- All props, callbacks, and controller state hooks are untouched.
- No changes to Tauri backend models or API integrations are made.
- The layout is backwards-compatible with existing configuration parameters and local storage states.
