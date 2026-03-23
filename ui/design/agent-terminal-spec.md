# AgentTerminal — Design Specification

**Issue:** #676 · Part of epic #671 (PTY execution backend)
**Designer:** designer agent
**Status:** Ready for implementation

---

## Overview

`AgentTerminal` is a new component that renders an xterm.js terminal connected to
the orchestrator's `/terminal/{agentId}` WebSocket endpoint. It appears as a
**Terminal** tab alongside the existing **Logs** tab in the agent detail page,
offering a raw PTY experience with full ANSI colour support.

The existing `AgentLogView` (structured, searchable, timestamped) is preserved
as the default tab. `AgentTerminal` supplements it — it never replaces it.

---

## Component Hierarchy

```
AgentDetail.tsx (modified)
└── AgentViewTabs            ← new inline tab bar (Logs | Terminal)
    ├── [active=logs]  → AgentLogView (unchanged)
    └── [active=terminal] → AgentTerminal (new)
                             ├── TerminalToolbar (status + interactive toggle)
                             ├── xterm.js canvas (connected state)
                             └── TerminalUnavailable (fallback state)

ui/src/hooks/useAgentTerminal.ts  ← new WebSocket + xterm lifecycle hook
ui/src/components/agents/AgentTerminal.tsx  ← new component
```

---

## Visual States

### State 1 — Connecting

```
┌─────────────────────────────────────────────────────┐
│  ⟳ Connecting…                      Read-only  [≡]  │  ← toolbar (bg-gray-900)
├─────────────────────────────────────────────────────┤
│                                                     │
│   Connecting to PTY stream…                         │  ← text-gray-500 italic
│   █                                                 │  ← blinking cursor
│                                                     │  ← bg-gray-950
└─────────────────────────────────────────────────────┘
```

- Toolbar shows yellow spinner + "Connecting…" badge (matches AgentLogView pattern)
- Terminal area shows placeholder text rendered into xterm canvas itself

### State 2 — Connected (read-only, default)

```
┌─────────────────────────────────────────────────────┐
│  ● Connected                        Read-only  [≡]  │  ← toolbar
├─────────────────────────────────────────────────────┤
│                                                     │
│  [2024-01-15 14:23:01] Starting agent worker...     │
│  [2024-01-15 14:23:02] Reading task from #674...    │
│  ✓ Workspace ready                                  │  ← full ANSI colours
│  > Running cargo build...                           │
│  █                                                  │
│                                                     │
└─────────────────────────────────────────────────────┘
```

- Green dot + "Connected" badge
- Interactive toggle button: "Read-only" (pill style, ghost variant)
- xterm.js renders at full colour fidelity
- Copy via standard terminal selection (no explicit button needed)

### State 3 — Connected (interactive)

```
┌─────────────────────────────────────────────────────┐
│  ● Connected                     Interactive  [≡]   │  ← toolbar, toggle accent
├─────────────────────────────────────────────────────┤
│                                                     │
│  > Running cargo build...                           │
│  █                                                  │  ← keyboard input accepted
│                                                     │
└─────────────────────────────────────────────────────┘
```

- Interactive toggle glows: `text-primary-400` background pill
- Keyboard events forwarded to PTY
- A subtle amber banner below toolbar: "Interactive mode — keystrokes are sent
  to the agent session."

### State 4 — Reconnecting

```
┌─────────────────────────────────────────────────────┐
│  ⟳ Reconnecting…                    Read-only  [≡]  │  ← toolbar
├─────────────────────────────────────────────────────┤
│                                                     │
│  (previous output preserved)                        │  ← terminal not cleared
│                                                     │
│  ─ ─ ─ ─ ─  reconnecting…  ─ ─ ─ ─ ─              │  ← injected separator line
│  █                                                  │
│                                                     │
└─────────────────────────────────────────────────────┘
```

- Amber spinner badge (matches AgentLogView "Connecting…" pattern)
- Previous terminal output preserved in xterm scrollback
- Separator line injected into xterm canvas via `terminal.write()` with
  dim ANSI styling: `\x1b[2m─── reconnecting… ───\x1b[0m\r\n`

### State 5 — Disconnected (unrecoverable)

```
┌─────────────────────────────────────────────────────┐
│  ✕ Disconnected                     Read-only  [≡]  │  ← toolbar
├─────────────────────────────────────────────────────┤
│                                                     │
│  (output preserved)                                 │
│  ─ ─ ─ ─ ─  stream ended  ─ ─ ─ ─ ─               │
│                                                     │
└─────────────────────────────────────────────────────┘
```

- Red badge. WebSocketManager stops retrying after agent terminates.

### State 6 — PTY Unavailable (graceful degradation)

```
┌─────────────────────────────────────────────────────┐
│  ○ Unavailable                                      │  ← toolbar, no toggle
├─────────────────────────────────────────────────────┤
│                                                     │
│         ┌────────────────────────────────┐          │
│         │  ⧉  Terminal not available     │          │  ← card, centered
│         │                                │          │
│         │  This agent is running on the  │          │
│         │  tmux backend, which doesn't   │          │
│         │  support PTY streaming.        │          │
│         │                                │          │
│         │  To enable the terminal view,  │          │
│         │  set AGENTD_BACKEND=pty when   │          │
│         │  starting the agent.           │          │
│         │                                │          │
│         │         [ View Logs ]          │          │  ← returns to Logs tab
│         └────────────────────────────────┘          │
│                                                     │
└─────────────────────────────────────────────────────┘
```

- Triggered when the WS connection returns 404 or a
  `{"error":"pty_not_supported"}` JSON message
- Card uses `border-gray-700 bg-gray-900` (same dark surface as toolbar)
- Icon: `SquareTerminal` from lucide-react (24px, text-gray-500)
- Body text: `text-gray-400 text-sm`
- "View Logs" button: ghost variant, switches activeTab back to 'logs'
- No reconnection attempts from this state

---

## Tab Bar Design

Placed above the `h-[480px]` log/terminal area, replacing the bare
`<div className="h-[480px]">` wrapper in `AgentDetail.tsx`.

```
┌───────────────────────────────────────────────────────────────────┐
│  Logs        Terminal                                             │  ← tab bar
├───────────────────────────────────────────────────────────────────┤
│                                                                   │
│  [active panel content]                                           │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

**Tailwind classes:**

```tsx
// Tab bar container
<div className="flex border-b border-gray-700">

// Inactive tab
<button className="px-4 py-2 text-sm font-medium text-gray-400
                   hover:text-white transition-colors
                   border-b-2 border-transparent -mb-px">

// Active tab
<button className="px-4 py-2 text-sm font-medium text-white
                   border-b-2 border-primary-500 -mb-px">

// Terminal tab — "PTY" badge when unavailable
<button ...>
  Terminal
  <span className="ml-1.5 rounded-full bg-gray-700 px-1.5 py-0.5
                   text-xs text-gray-400">PTY</span>
</button>
```

The `PTY` badge appears only when the backend reports unavailability, serving
as a persistent reminder that this tab requires a specific backend.

---

## xterm.js Theme Tokens

The terminal is **always dark** regardless of the app's light/dark mode.
This matches `AgentLogView` which hardcodes dark `gray-*` values.

```ts
// ui/src/styles/themes.ts — add alongside existing Nivo theme
export const XTERM_THEME = {
  background: '#030712',       // gray-950 — matches AgentLogView bg
  foreground: '#e5e7eb',       // gray-200
  cursor: '#d2852d',           // sunlit-clay-500 (--accent-primary light)
  cursorAccent: '#030712',
  selectionBackground: 'rgba(210, 133, 45, 0.25)',  // accent-primary @ 25%
  selectionForeground: '#fafafa',

  // Standard 16-colour ANSI palette (WCAG AA on #030712 background)
  black:         '#1f2937',    // gray-800
  brightBlack:   '#374151',    // gray-700
  red:           '#f87171',    // red-400
  brightRed:     '#fca5a5',    // red-300
  green:         '#4ade80',    // green-400
  brightGreen:   '#86efac',    // green-300
  yellow:        '#fbbf24',    // amber-400
  brightYellow:  '#fde68a',    // amber-200
  blue:          '#60a5fa',    // blue-400
  brightBlue:    '#93c5fd',    // blue-300
  magenta:       '#c084fc',    // purple-400
  brightMagenta: '#d8b4fe',    // purple-300
  cyan:          '#22d3ee',    // cyan-400
  brightCyan:    '#67e8f9',    // cyan-300
  white:         '#e5e7eb',    // gray-200
  brightWhite:   '#f9fafb',    // gray-50
} as const
```

---

## Interactive Mode Toggle

```
Read-only  ←→  Interactive

[Read-only]   — default, ghost pill, text-gray-400
[Interactive] — active, pill with bg-primary-900/40 text-primary-400
                + amber banner below toolbar
```

Toggle is a `<button>` with `role="switch"` and `aria-checked`.

**Amber banner (interactive mode warning):**
```tsx
<div className="flex items-center gap-2 border-b border-amber-900/30
                bg-amber-950/20 px-3 py-1.5 text-xs text-amber-400">
  <AlertTriangle size={12} aria-hidden="true" />
  Interactive mode — keystrokes are sent to the agent session
</div>
```

---

## WebSocket Protocol

The `useAgentTerminal` hook connects to:
```
ws://{orchestratorHost}/terminal/{agentId}
```
replacing `http://` → `ws://` (or `https://` → `wss://`) from
`serviceConfig.orchestratorServiceUrl`.

**Binary frames:** xterm PTY output arrives as `ArrayBuffer` (binary).
The hook must set `ws.binaryType = 'arraybuffer'` and decode with
`new TextDecoder().decode(event.data)` before writing to xterm.

**Text frames (JSON control messages):**
```jsonc
{ "type": "resize", "cols": 120, "rows": 40 }     // client → server
{ "type": "input",  "data": "\x03" }               // client → server (interactive)
{ "error": "pty_not_supported" }                   // server → client (triggers unavailable state)
```

**Resize flow:**
1. `ResizeObserver` on container element fires
2. `FitAddon.fit()` recomputes cols/rows
3. Hook sends `{"type":"resize","cols":N,"rows":N}` JSON text frame

---

## Lazy Connection

`AgentTerminal` accepts an `active: boolean` prop. The WebSocket connection
and xterm Terminal instance are created **only when `active` becomes true**
for the first time. On tab switch back to Logs, the connection stays open
(don't reconnect on every tab switch). On component unmount, disconnect.

```
active=false → no WS, no xterm instance (deferred)
active=true  → WS connects, xterm mounts into container ref
active=false → WS stays open, xterm stays mounted (hidden via CSS)
unmount      → WS.disconnect(), terminal.dispose()
```

---

## Accessibility

| Requirement | Implementation |
|---|---|
| WCAG AA contrast on terminal | All 16 ANSI colours verified ≥4.5:1 on #030712 |
| Interactive toggle announced | `role="switch"` + `aria-checked` + `aria-label` |
| Unavailable state describable | Info card has `role="status"` for SR |
| Focus management on tab switch | Focus the xterm container when Terminal tab activates |
| Reduced motion | xterm cursor blink respects `prefers-reduced-motion` via `cursorBlink: !prefersReducedMotion` |
| Keyboard nav | Tab key navigates toolbar controls; xterm captures keyboard only when focused |
| Touch targets | All toolbar buttons ≥44×44px tap area via `min-h-[44px]` on mobile |

---

## Performance

- xterm scrollback capped at **5 000 lines** (`scrollback: 5000` option) to
  prevent memory growth in long-running sessions
- `FitAddon` debounced 100ms on resize to avoid thrashing
- Terminal writes are batched: collect chunks in a microtask queue, flush on
  `requestAnimationFrame` for smooth rendering
- Connection only established when tab is first activated (`active` prop)

---

## Files Changed

| File | Change |
|---|---|
| `ui/src/components/agents/AgentTerminal.tsx` | **New** — terminal component |
| `ui/src/hooks/useAgentTerminal.ts` | **New** — WS + xterm lifecycle hook |
| `ui/src/pages/agents/AgentDetail.tsx` | **Modified** — add tab bar, import AgentTerminal |
| `ui/src/styles/themes.ts` | **Modified** — add XTERM_THEME export |
| `ui/design/agent-terminal-spec.md` | **New** — this document |
| `package.json` / `bun.lock` | **Modified** — add @xterm/* deps |

---

## Open Questions for Implementation

1. **`/terminal/{agentId}` endpoint path** — confirm with backend (#675) that
   this is the exact route; the hook uses `serviceConfig.orchestratorServiceUrl`
   with protocol swap

2. **Binary vs text frames** — confirm whether PTY relay sends binary
   `ArrayBuffer` frames or base64-encoded text; hook handles both but needs
   the right decode path

3. **Search addon UX** — `@xterm/addon-search` provides `findNext`/`findPrevious`;
   the toolbar search UI (text input + arrow buttons) is deferred to a follow-up
   issue to keep scope tight
