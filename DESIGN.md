---
name: NEXUS OS
description: A spatial desktop operating system lit by a real kernel; the wallpaper's light and every widget's number come from the running Rust kernel.
colors:
  ink: "#0a0e1a"
  ground-mid: "#0e1328"
  ground-deep: "#141a35"
  glass: "rgba(20, 24, 44, .72)"
  glass-focused: "rgba(26, 31, 56, .72)"
  glass-menu: "rgba(22, 26, 48, .78)"
  glass-widget: "rgba(18, 22, 42, .5)"
  glass-dock: "rgba(20, 24, 46, .45)"
  glass-bar: "rgba(10, 13, 26, .55)"
  glass-light: "rgba(245, 246, 255, .95)"
  well: "rgba(4, 6, 14, .45)"
  raise: "rgba(255, 255, 255, .07)"
  raise-hover: "rgba(255, 255, 255, .13)"
  rule: "rgba(255, 255, 255, .08)"
  rule-2: "rgba(255, 255, 255, .16)"
  chalk: "#eef0ff"
  chalk-2: "rgba(232, 236, 255, .8)"
  slate: "#a3abc9"
  accent: "#8b9cff"
  accent-hi: "#a3b1ff"
  on-accent: "#0b0f22"
  coral: "#ff8a65"
  coral-hi: "#ffa284"
  cyan: "#5ed3f3"
  mint: "#6ee7b7"
  amber: "#ffc46b"
  rose: "#ff6b8b"
  violet: "#a78bfa"
  pink: "#f472b6"
  sky: "#7daaff"
  lime: "#bee678"
  task-dim: "rgba(255, 255, 255, .22)"
typography:
  display:
    fontFamily: "Onest, Segoe UI, system-ui, sans-serif"
    fontSize: "104px"
    fontWeight: 200
    lineHeight: 1
    letterSpacing: "-0.04em"
  headline:
    fontFamily: "Onest, Segoe UI, system-ui, sans-serif"
    fontSize: "24px"
    fontWeight: 300
    lineHeight: 1.2
  date:
    fontFamily: "Onest, Segoe UI, system-ui, sans-serif"
    fontSize: "18px"
    fontWeight: 400
    lineHeight: 1.4
  section:
    fontFamily: "Onest, Segoe UI, system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 500
    lineHeight: 1.3
    letterSpacing: "-0.005em"
  title:
    fontFamily: "Onest, Segoe UI, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 600
    lineHeight: 1.3
  body:
    fontFamily: "Onest, Segoe UI, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "Onest, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.3
  figure:
    fontFamily: "Spline Sans Mono, ui-monospace, Consolas, monospace"
    fontSize: "20px"
    fontWeight: 500
    lineHeight: 1
    fontFeature: "tnum"
  data:
    fontFamily: "Spline Sans Mono, ui-monospace, Consolas, monospace"
    fontSize: "13.5px"
    fontWeight: 400
    lineHeight: 1.35
    fontFeature: "tnum"
  data-small:
    fontFamily: "Spline Sans Mono, ui-monospace, Consolas, monospace"
    fontSize: "12.5px"
    fontWeight: 400
    lineHeight: 1
    fontFeature: "tnum"
rounded:
  hairline: "7px"
  control: "9px"
  tile: "12px"
  window: "14px"
  panel: "18px"
  dock: "24px"
  pill: "999px"
spacing:
  tight: "6px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  section: "22px"
  desk-edge: "44px"
  bar: "32px"
  dock: "86px"
components:
  menubar:
    backgroundColor: "{colors.glass-bar}"
    textColor: "{colors.chalk-2}"
    height: "{spacing.bar}"
    typography: "{typography.label}"
  status-capsule:
    backgroundColor: "rgba(0, 0, 0, .42)"
    textColor: "{colors.chalk-2}"
    rounded: "{rounded.pill}"
    height: "24px"
    padding: "0 12px 0 10px"
    typography: "{typography.data-small}"
  menu:
    backgroundColor: "{colors.glass-menu}"
    rounded: "{rounded.window}"
    padding: "6px"
  menu-item:
    textColor: "{colors.chalk-2}"
    rounded: "8px"
    padding: "8px 10px"
    typography: "{typography.body}"
  menu-item-hover:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.on-accent}"
  control-center-tile:
    backgroundColor: "{colors.raise}"
    rounded: "{rounded.tile}"
    padding: "12px"
  policy-segment-on:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.on-accent}"
    rounded: "8px"
    height: "30px"
  widget:
    backgroundColor: "{colors.glass-widget}"
    textColor: "{colors.chalk-2}"
    rounded: "{rounded.panel}"
    padding: "13px 15px 12px"
    width: "292px"
  dock:
    backgroundColor: "{colors.glass-dock}"
    rounded: "{rounded.dock}"
    padding: "8px 10px"
  dock-tile:
    backgroundColor: "{colors.raise}"
    rounded: "{rounded.window}"
    size: "52px"
  window:
    backgroundColor: "{colors.glass}"
    textColor: "{colors.chalk}"
    rounded: "{rounded.window}"
  window-focused:
    backgroundColor: "{colors.glass-focused}"
  window-bar:
    textColor: "{colors.chalk-2}"
    height: "42px"
    padding: "0 8px 0 14px"
    typography: "{typography.title}"
  button:
    backgroundColor: "rgba(255, 255, 255, .06)"
    textColor: "{colors.chalk}"
    rounded: "{rounded.control}"
    height: "32px"
    padding: "0 14px"
  button-hover:
    backgroundColor: "{colors.raise-hover}"
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.on-accent}"
    rounded: "{rounded.control}"
    height: "32px"
    padding: "0 14px"
  button-primary-hover:
    backgroundColor: "{colors.accent-hi}"
  button-danger:
    backgroundColor: "{colors.rose}"
    textColor: "#ffffff"
    rounded: "{rounded.control}"
  input:
    backgroundColor: "{colors.well}"
    textColor: "{colors.chalk}"
    rounded: "{rounded.control}"
    padding: "7px 10px"
  chip:
    textColor: "{colors.slate}"
    rounded: "{rounded.pill}"
    height: "26px"
    padding: "0 8px"
  chip-on:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.on-accent}"
  spotlight:
    backgroundColor: "rgba(22, 26, 50, .8)"
    rounded: "{rounded.panel}"
    width: "620px"
  lock-card:
    backgroundColor: "rgba(14, 17, 34, .72)"
    rounded: "{rounded.panel}"
    padding: "16px 18px"
  toast:
    backgroundColor: "{colors.glass-light}"
    textColor: "{colors.on-accent}"
    rounded: "{rounded.tile}"
    padding: "9px 14px"
---

# Design System: NEXUS OS

## Overview

**Creative North Star: "The Kernel Twilight"**

NEXUS OS is a spatial desktop at dusk. A deep twilight ground carries a slow generative aurora, and the aurora's light is the kernel itself. Each running task is a soft field of colour whose size and brightness follow its real CPU share. A dispatch makes its field flare, a kill flashes rose from where the task was, and the heap in use glows along the horizon. With no kernel, the sky stays dark. Over that sky float layered panes of dark, blurred glass: a translucent menu bar with a live kernel capsule, four kernel widgets, a magnifying dock, and windows that open with a soft settle.

The system is calm and premium, but it is never decorative. Every number on screen is a reading from the live kernel trace (`#T SNAP`, `HEAPMAP`, `DISPATCH`, `OFF`, `WAKE`, `SPAWN`, `EXIT`, the boot trace) or from a real host action (a cargo build, a file on disk, a test log). The glass holds instruments, not ornaments. Density is moderate. The desktop level is generous, with a large thin clock and wide margins. Inside windows the layout is compact and tabular, and Spline Sans Mono carries the figures.

Confirmed rejections: the discarded ASTRA and carbon/coral "kernel topography" NEXUS looks (none of their tokens carry forward), a photo wallpaper with decorative widgets, a centred lock clock in a classmate's style, and the generic dashboard.

**Key Characteristics:**
- A living aurora wallpaper driven only by scheduler and heap data.
- Dark translucent glass at 22-40px blur, with a hairline inner highlight and soft, layered shadows.
- One electric periwinkle accent for focus, selection and the primary action.
- Task colours that mean the same task everywhere: aurora, scheduler ribbon, charts and legends.
- Onest for words; Spline Sans Mono, with tabular figures, for kernel numbers.
- Ease-out motion, all of it off under reduced motion.

## Colors

A twilight ink ground, white-tinted glass, one periwinkle accent, and a set of task lights in which three saturated leads carry the scene.

### Primary
- **Electric Periwinkle** (#8b9cff): the one system accent. It marks focus rings (2px outline), text selection (at 45%), the highlighted menu item and Spotlight hit, the selected scheduler policy, the focused window's icon, the focused dock tile (22% fill, 55% border), the Mission Control hover ring, primary buttons, active chips and tabs, the slider tint and the terminal cursor. Text on it is near-black **On-Accent Ink** (#0b0f22). On hover it lifts to **Periwinkle Light** (#a3b1ff).

### Secondary: state colours
- **Running Coral** (#ff8a65): a running task (process status dot), the CPU meter in the menu bar, CPU history bars above 85%, task spawn and exit in the Journal, editor keywords and caret, and a build in progress. On hover it lifts to #ffa284.
- **Ready Mint** (#6ee7b7): ready and ok. It marks the live dot in the status capsule, the connection dot, the heap meter, heap dots in use, wake-ups and policy changes in the Journal, passing checks, a successful build, "exited, code 0", and directories in Files. The stylesheet still names this variable `--celadon`; the value and the role are mint.
- **Waiting Amber** (#ffc46b): the middle load band (CPU history bars between 40% and 85%) and the terminal's yellow.
- **Kill Rose** (#ff6b8b): kills, failures and errors only. It marks failed checks, negative syscall returns, compiler errors, killed programs, the close button on hover, destructive buttons, and the kernel-kill flash on the aurora (rgba(255, 80, 110)).

### Tertiary: task lights
- **Lead Coral** (#ff8a65), **Lead Cyan** (#5ed3f3), **Lead Mint** (#6ee7b7): the three lead slots, owned by the three busiest tasks.
- **Quiet hues**: Amber (#ffc46b), Violet (#a78bfa), Pink (#f472b6), Sky (#7daaff) and Lime (#bee678). Every other task takes one of these in charts and legends, chosen by task id.
- **Dim Task** (rgba(255, 255, 255, .22)): non-lead tasks in the scheduler ribbon. The "others" key swatch is white at 30%.
- **Heap Horizon** (rgba(110, 140, 255)): the glow along the bottom of the aurora, from 18% to 50% alpha as heap use grows.

### Neutral
- **Twilight Ink** (#0a0e1a): the page ground, and text on light fills. The aurora's sky runs from #090c18 at the top through **Dusk** (#0e1328) to **Deep Twilight** (#141a35) at the horizon.
- **The glass ladder**: **Window Glass** (rgba(20, 24, 44, .72)), **Focused Glass** (rgba(26, 31, 56, .72)), **Menu Glass** (rgba(22, 26, 48, .78)), **Widget Glass** (rgba(18, 22, 42, .5)), **Dock Glass** (rgba(20, 24, 46, .45)) and **Bar Glass** (rgba(10, 13, 26, .55)). Surfaces you read in are the most opaque; surfaces you glance at are the most transparent.
- **Well** (rgba(4, 6, 14, .45)): recessed areas inside glass, such as inputs, the editor, file previews and logs. The terminal body uses .55.
- **Raise** (white at 7%) and **Raise Hover** (white at 12-13%): tiles, buttons and menu-bar hover.
- **Chalk** (#eef0ff): primary text and figures. **Chalk Soft** (rgba(232, 236, 255, .8)): body text and menu items. **Slate** (#a3abc9): secondary text, labels, and icons at rest (6.8:1 on window glass).
- **Rule** (white at 8%) and **Rule Strong** (white at 16%): dividers, table rules and separators. Glass edges use white at 10-14%.
- **Light Glass** (rgba(245, 246, 255, .95)): the only light surface. The toast and the guided tour use it so they read as messages rather than places.

### Named Rules
**The Lead Light Rule.** The three busiest tasks (above 2% CPU share) own Lead Coral, Lead Cyan and Lead Mint. A task keeps its slot, and so its colour, for as long as it stays in the top three. A freed slot goes to the next newcomer without reshuffling the others. On the aurora a lead field is large and saturated (up to 62% alpha). Every other task is a small dim haze (at most 14% alpha, shrinking as tasks multiply), so many tasks never add up to white. In the ribbon, non-lead tasks are dim white.

**The One Accent Rule.** Periwinkle is the only interaction colour. State colours report kernel state and are never used for selection, focus or the primary action.

**The Rose Means Death Rule.** Rose appears only for kills, failures and errors, never for emphasis or decoration.

## Typography

**Display Font:** Onest (with Segoe UI, system-ui)
**Body Font:** Onest (with Segoe UI, system-ui)
**Label/Mono Font:** Spline Sans Mono (with ui-monospace, Consolas)

**Character:** Onest is a clean, slightly humanist grotesque that runs from a whisper-thin 200 clock to a firm 600 title. Spline Sans Mono is a quiet geometric mono that sits beside it for every kernel figure. Both are self-hosted woff2 variable fonts (Onest 200-700, Spline Sans Mono 300-700). Tabular numerals are on globally.

### Hierarchy
- **Display** (200, 104px, line-height 1, -0.04em): the desktop clock and the lock-screen clock, in white. On phones the lock clock is 84px and the desktop clock 72px.
- **Headline** (300, 24px, 1.2): the Spotlight query. The System app's About title is the one heavier exception (600, 34px, -0.02em).
- **Date** (400, 18px, 1.4): the date under the clock.
- **Section** (500, 16px, 1.3): block headings inside windows. Dialog titles are 500 at 17px.
- **Title** (600, 14px, 1.3): window titles. The menu bar's focused-app name is 600 at 13.5px, and widget and Control Center headings are 600 at 13-13.5px.
- **Body** (400, 14px, 1.5): running text, menu items and table names. Prose is capped at 68ch.
- **Label** (400, 13px): hints, subtitles and captions, in Slate.
- **Figure** (Spline Sans Mono 500, 20px, line-height 1): the headline number of each widget.
- **Data** (Spline Sans Mono 400, 13.5px, 1.35): tables, readouts, and the editor (at 1.6 line-height).
- **Data small** (Spline Sans Mono 400, 12.5px): the status capsule, menu-bar meters, uptime, policy buttons and shortcuts.

### Named Rules
**The Two Voices Rule.** Words are Onest. Anything the kernel counts (ticks, pids, percentages, sizes, times, return values, and policy names in controls) is Spline Sans Mono with tabular figures.

**The Thin Clock Rule.** Weight 200 belongs to the clock alone. Nothing else on the desktop goes below 300.

## Layout

The screen is a 32px menu bar over a full-bleed workspace, and the aurora fills the workspace. The desktop clock sits at the upper left (44px in, 36px down). Under it are the date, a mono uptime line (`kernel up 00:01:06 · 7 tasks · Round Robin`) and a caption naming the brightest light on the wallpaper. A 292px column of four widgets sits 20px from the top-right corner, with a 12px gap. The dock floats centred 10px above the bottom edge, and maximised windows stop above it (the dock reserves 86px). The lock screen puts its clock in the same position, so unlocking is one continuous scene.

Windows float freely. They drag by a 42px title bar and resize by a corner grip. Their content is padded 16px 18px 20px, with 22px between sections, in auto-fit two-column grids (260px minimum) that collapse through container queries. Popovers anchor 4px below the menu bar: the system menu at the left (300px), app menus under their titles (240px), and Control Center at the right (360px). The capsule drops its widest fields below 1600px, and its tick count and the heap meter below 1280px, so it never touches the menus.

**Phone layout (820px wide or less).** One window at a time:
- **Windows:** the focused window fills the screen above the dock, with square corners. The others are hidden, and maximise and resize are removed.
- **Menu bar:** it keeps the mark, the app name, a shortened capsule (policy and quantum), the CPU figure, Control Center and notifications. The app menus, heap meter, Spotlight button and date are hidden.
- **Desktop:** the clock drops to 72px, 20px from the edge. The widgets become a full-width stack above the dock: CPU, Scheduler and Memory (System calls is hidden).
- **Dock:** it spans the screen width and wraps its 44px tiles (radius 11px) onto two rows. It loses its separators, tooltips and magnification, and reserves 128px.
- **Popovers and splits:** popovers and notifications span the width with 6-8px margins. Files and the Kernel Map stack their split panes vertically.

## Elevation & Depth

Depth is glass over light. Every surface is translucent, blurred and saturated (backdrop blur with 140-180% saturation), so the aurora shows through. Each pane carries a 1px white hairline edge and a 1px inner top highlight. Shadows are soft, long and dark, layered in pairs, and never hard-edged. Blur rises with how much a surface asks for attention:

| Surface | Blur |
|---|---|
| Lock scrim | 22px |
| Widgets | 26px |
| Generic glass | 28px |
| Windows, menu bar, dock, notifications | 30px |
| Menus | 32px |
| Spotlight | 40px |

The Mission Control scrim and the dialog backdrop use a light 4-6px blur over a dark tint.

### Shadow Vocabulary
- **Pane** (`box-shadow: 0 30px 70px -24px rgba(0,0,0,.6), 0 12px 28px -12px rgba(0,0,0,.45), inset 0 1px 0 rgba(255,255,255,.07)`): windows, menus, the lock card, notifications, the toast and dialogs.
- **Focused pane** (`box-shadow: 0 40px 90px -28px rgba(0,0,0,.7), 0 14px 32px -14px rgba(0,0,0,.5), inset 0 1px 0 rgba(255,255,255,.1)`): the focused window, which also brightens its glass and its edge (white at 17%).
- **Widget** (`box-shadow: 0 18px 40px -22px rgba(0,0,0,.55), inset 0 1px 0 rgba(255,255,255,.06)`): desktop widgets.
- **Dock** (`box-shadow: 0 20px 50px -20px rgba(0,0,0,.6), inset 0 1px 0 rgba(255,255,255,.08)`): the dock. Each tile adds `inset 0 1px 0 rgba(255,255,255,.14), 0 8px 16px -8px rgba(0,0,0,.6)`.
- **Spotlight** (`box-shadow: 0 40px 100px -30px rgba(0,0,0,.8)`): the deepest float.

### Named Rules
**The Aurora Shows Through Rule.** No large surface is opaque. A pane that hides the sky completely has left the world. The wells inside windows are the only near-solid areas, and even they are 45-55% alpha.

**The Focus Lifts Rule.** Focus is shown by depth and tint together. The focused window gets more glass, a brighter edge, a deeper shadow and a periwinkle icon; unfocused windows recede.

## Shapes

Corners are soft and continuous, and they scale with the size of the surface:

| Radius | Used for |
|---|---|
| 7px | Menu-bar hit areas, tooltips, small actions |
| 8px | Menu items, policy segments |
| 9px | Buttons, inputs |
| 12px | Control Center tiles, the toast, grouped bands |
| 14px | Windows, menus, notifications, dock tiles, research cards |
| 18px | Widgets, the lock card, Spotlight, dialogs, the tour |
| 24px | The dock |
| Full pill | The status capsule, chips, notification badges |
| Circle | Window controls (26px), status dots, the avatar, toggle icons |

Maximised windows and phone windows drop to square corners because they meet the screen edge. Heap cells are squares rounded 3px in the Memory app and dots in the widget. Icons are original line drawings with a 1.6px stroke and round caps.

## Components

### Menu bar and status capsule
Quiet, always-present glass: Bar Glass with a 30px blur and a white 6% bottom rule, 32px tall.
- **Left:** the NEXUS mark (it opens the system menu), the focused app's name in 600 weight, then its menus (Run, Window, Kernel, Help) as 24px hit areas. A menu takes Raise Hover on hover or while open.
- **Centre:** the **status capsule**, a 24px pill in black at 42%. It shows a mint breathing dot while the kernel runs, then the policy, quantum, ticks and task count in Data small, separated by 12px hairlines. Clicking it opens Control Center.
- **Right:** CPU (coral) and heap (mint) mini meters (34 x 5px) with mono figures, then the Spotlight, Control Center and notification icons, the connection dot and the clock.

### Menus
Menu Glass at 32px blur, with a 14px radius, 6px padding and the Pane shadow; menus open with the `pop` settle. Items are 8px-rounded rows with a 17px Slate icon. On hover or keyboard focus, the row fills with periwinkle and its text and icon turn On-Accent Ink. Shortcuts align right in Data small Slate, and separators are Rule lines inset 6px. The system menu starts with the machine's identity and an app search.

### Control Center
A 360px two-column panel of 12px-rounded Raise tiles.
- **Scheduler tile (full width):** the eight policies as a 4 x 2 segmented radio group (30px high, mono 500 12px); the selected policy fills periwinkle. Below them sit a periwinkle quantum slider with its mono value and the primary action, "Reboot with this scheduler", which performs a real VM reboot.
- **Toggle tiles:** Living wallpaper, Do not disturb, Mission Control, and Power off the QEMU machine. Each has a 30px round icon that fills periwinkle when on.

### Dock
Dock Glass at 30px blur, with a 24px radius and 8px 10px padding, centred above the bottom edge. It holds ten 52px tiles (white at 7%, 14px radius, inset highlight) with 26px line icons, grouped by two 1px separators.
- **Running and focused apps:** a running app gets a 4px white dot (70%) beneath it. The focused app gets a full white dot and a periwinkle-tinted tile.
- **Magnification:** tiles within 140px of the pointer grow up to 1.42x, with up to 10px of extra spacing, and tooltips rise 10px above. Magnification is off under reduced motion and on phones.

### Widgets
Four live instruments. Each is a Widget Glass card (18px radius, 26px blur) that opens its app on click and lifts 2px on hover. The header pairs a 600 13px name on the left with a Figure-sized mono reading on the right.
- **CPU**: 60 one-second bars of the hart's busy share, coloured by load (mint, then amber above 40%, coral above 85%), with older bars faded.
- **Scheduler**: the signature ribbon. It shows the last 3 s of real dispatches in quarter-second columns, each stacked by task share in 3px-rounded bands. Lead tasks take their lead colours and all others are dim white. A key names the leads by name and pid.
- **Memory**: the kernel heap as a 32-column field of dots, mint where a slice is in use, with the slice count and heap size beneath.
- **System calls**: the last four real calls in mono (pid, call, return value), in rose when the return is negative.

### Windows
Window Glass with a 14px radius, a white 10% edge and the Pane shadow. A focused window takes Focused Glass, a 17% edge and the deeper shadow.
- **Title bar (42px):** a 17px app icon (periwinkle when focused), the title in Title weight (white when focused), and a Slate subtitle that states truthfully what the window shows ("project files on the host · the kernel has no file system"). Three 26px round controls sit at the right: minimise, maximise and close. Close turns rose on hover.
- **Body:** toolstrips sit under the title bar on a Rule line. Content wells (terminal, editor, previews) use the Well tint.
- **Buttons:** 9px-rounded glass. The primary button is periwinkle, and destructive confirmations are rose.

### Mission Control
Opened from the Window menu, Control Center or Ctrl+F3. A dark scrim (rgba(4, 6, 14, .45), 6px blur) falls over the desktop, and every open window moves into a grid with a 36px gutter, scaled to fit, over 450ms with the settle curve. A label pill (Menu-like glass at 80%, 8px radius) names each window beneath it, and hover rings a window in 3px periwinkle. Click, Enter or Space focuses a window; Escape closes Mission Control.

### Spotlight
A 620px glass sheet (rgba(22, 26, 50, .8), 40px blur, 18px radius) placed at 18% of the viewport height. It has a 22px search icon and a light 24px query. Results list apps and shell commands as 10px-rounded rows with a Slate secondary line, and the selected or hovered row fills periwinkle. A 12px Slate footer line explains the keys.

### Lock screen
A 22px-blur scrim over the live aurora. The clock and date sit exactly where the desktop clock sits. Below them is a lock card (18px radius) holding:
- the avatar, a periwinkle-tinted 64px circle;
- the machine identity in mono (`Micro-OS kernel v0.1.0 · riscv64 · QEMU virt · 1 hart · 128 MiB`);
- the real boot time from the boot trace;
- a periwinkle Enter button.

Unlocking lifts the scene by 3% while blurring it out over 550ms.

### Notifications and toast
Notifications are 14px-rounded Menu Glass cards that slide in from the right with a tone icon: rose for kernel kills, mint for success. They are hidden while Control Center or Mission Control is open. The toast and the guided tour are the only Light Glass surfaces, with near-black text.

## Do's and Don'ts

### Do:
- **Do** drive every number, colour field and state on screen from the live kernel trace or a real host action. When the kernel is off, leave the sky dark and the readings blank.
- **Do** give the three busiest tasks the lead colours and let each one keep its colour while it stays in the top three; draw everything else dim.
- **Do** use Spline Sans Mono with tabular figures for every kernel number, and Onest for words.
- **Do** build new surfaces as translucent glass with a hairline edge, an inner top highlight and a soft layered shadow, with blur scaled to the surface's importance (26-40px).
- **Do** use periwinkle (#8b9cff) as the single interaction colour, with On-Accent Ink (#0b0f22) on it.
- **Do** move with the settle curve, `cubic-bezier(.22, 1, .36, 1)` (ease-out), at 120-550ms for interface motion. The only looping motion is the capsule's live dot, which breathes over 2.4s.
- **Do** switch off every transition, animation, aurora drift and dock magnification under `prefers-reduced-motion`. In that mode the aurora jumps straight to each new reading, and the lock screen leaves at once.
- **Do** label what is not in the kernel as what it is: host files, host builds, the separate Debian distribution, and wake-ups (never IPC).

### Don't:
- **Don't** carry forward any token from the discarded ASTRA or carbon/coral/celadon topography worlds (olive-cream grounds, teal-carbon panels, square-cornered surfaces).
- **Don't** use a photo or static image as the wallpaper, or add decorative widgets that show nothing real.
- **Don't** use a state colour (coral, mint, amber, rose) for selection, focus or a primary action.
- **Don't** use rose for anything except kills, failures and errors.
- **Don't** make a large surface fully opaque; the aurora has to show through.
- **Don't** use bouncing, overshooting or ease-in motion.
- **Don't** show a mocked, simulated or invented value anywhere, including in placeholders and demos.
