# Surface brief: NEXUS OS desktop (frontend/static/index.html)

Scope: the whole browser desktop (boot, lock screen, desktop, all apps). Mode: Operate, code-led.
Audience and job: the student demonstrating the real Rust kernel in a viva (laptop 1536x864, projector) and developing on it; the examiner must believe within seconds that this is a real, polished operating system running a real kernel.
Constraints: every value from the live kernel or a real host action; no IPC, no in-kernel FS/compiler, no microkernel claims; offline; no copied assets.

## Direction contract

THESIS: A spatial premium desktop OS whose living surfaces are driven by a real kernel: the wallpaper is a slow generative aurora whose light comes from the tasks the scheduler is running, and every desktop widget is a working kernel instrument. It refuses the photo-wallpaper-plus-decorative-widgets desktop and the dashboard.

OWN-WORLD: Deep twilight ground (ink #0a0e1a to #141a33) lit by soft, blurred colour fields, one per live task (coral, amber, cyan, violet, mint), brighter as its CPU share grows. Layered translucent glass panels (dark glass, 28px blur, a hairline inner highlight, 14-18px radii, layered soft shadows for depth). Onest for interface and display (thin 200 for the clock, 500 for titles), Spline Sans Mono for kernel numbers (amended from Geist Mono, which the detector flags as an overused default; Spline Sans Mono keeps the same quiet geometric voice beside Onest). One system accent: electric periwinkle #8b9cff for focus and selection; state colours: coral running, mint ready/ok, amber waiting, rose for kills and failures.

STORY: Boot shows the kernel's real boot stages; a lock screen greets with the time and the machine's identity; the desktop opens with a live clock, kernel widgets (CPU history, scheduler ribbon of the real dispatch order, heap dots, last system calls) over the aurora; apps open as spatial windows from a magnifying dock; Control Center switches the scheduler (real reboot); Mission Control spreads every window; Spotlight runs apps and shell commands; notifications report kernel kills.

FIRST VIEWPORT: 32px translucent menu bar (NEXUS mark + focused app + app menu; centre kernel status capsule: policy · quantum · ticks · running; right: CPU, heap, Control Center, notifications, clock). Desktop: large thin clock and date upper-left with uptime beneath; a column of four glass widgets on the right; the aurora behind; glass dock centred at the bottom with original app icons and running dots. No window open on first view after the lock screen; Terminal opens from the dock or Spotlight.

FORM: user-chosen "Spatial premium OS" (asked after the dealt hand: Swiss poster OS [assigned], Braun appliance OS [pick], transit-map OS [challenger], standard desktop [canon], which the user declined). seed:ec4070862690caac. Signature move: the kernel aurora wallpaper plus the scheduler ribbon widget (the last 2 s of real dispatches as a coloured ribbon). Motion grammar: spring-eased window open/minimise to the dock, Mission Control spread, aurora drifting slowly with each snapshot; all off under reduced motion.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
