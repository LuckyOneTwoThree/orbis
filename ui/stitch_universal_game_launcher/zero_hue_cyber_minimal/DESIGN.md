---
name: Zero-Hue Cyber Minimal
colors:
  surface: '#131313'
  surface-dim: '#131313'
  surface-bright: '#3a3939'
  surface-container-lowest: '#0e0e0e'
  surface-container-low: '#1c1b1b'
  surface-container: '#201f1f'
  surface-container-high: '#2a2a2a'
  surface-container-highest: '#353534'
  on-surface: '#e5e2e1'
  on-surface-variant: '#bac9cc'
  inverse-surface: '#e5e2e1'
  inverse-on-surface: '#313030'
  outline: '#849396'
  outline-variant: '#3b494c'
  surface-tint: '#00daf3'
  primary: '#c3f5ff'
  on-primary: '#00363d'
  primary-container: '#00e5ff'
  on-primary-container: '#00626e'
  inverse-primary: '#006875'
  secondary: '#e1b6ff'
  on-secondary: '#4c007b'
  secondary-container: '#691a9f'
  on-secondary-container: '#d69eff'
  tertiary: '#ffe7e6'
  on-tertiary: '#680013'
  tertiary-container: '#ffc1c0'
  on-tertiary-container: '#b4012a'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#9cf0ff'
  primary-fixed-dim: '#00daf3'
  on-primary-fixed: '#001f24'
  on-primary-fixed-variant: '#004f58'
  secondary-fixed: '#f2daff'
  secondary-fixed-dim: '#e1b6ff'
  on-secondary-fixed: '#2e004d'
  on-secondary-fixed-variant: '#691a9f'
  tertiary-fixed: '#ffdad9'
  tertiary-fixed-dim: '#ffb3b2'
  on-tertiary-fixed: '#410008'
  on-tertiary-fixed-variant: '#920020'
  background: '#131313'
  on-background: '#e5e2e1'
  surface-variant: '#353534'
  bg-base: '#050505'
  surface-1: '#0D0D0D'
  surface-2: '#141414'
  surface-3: '#1E1E1E'
  border-hairline: rgba(255, 255, 255, 0.08)
  text-primary: '#FFFFFF'
  text-secondary: rgba(255, 255, 255, 0.60)
  text-disabled: rgba(255, 255, 255, 0.30)
  glow-cyan: rgba(0, 229, 255, 0.35)
  glow-fuchsia: rgba(199, 125, 255, 0.30)
typography:
  headline-xl:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: '600'
    lineHeight: 32px
    letterSpacing: -0.02em
  headline-lg:
    fontFamily: Inter
    fontSize: 20px
    fontWeight: '600'
    lineHeight: 28px
    letterSpacing: -0.015em
  headline-md:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: '600'
    lineHeight: 24px
    letterSpacing: -0.01em
  body-lg:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: '400'
    lineHeight: 24px
  body-md:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: '400'
    lineHeight: 20px
  body-sm:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: '400'
    lineHeight: 16px
  label-md:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: '500'
    lineHeight: 20px
    letterSpacing: 0.01em
  label-sm:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: '500'
    lineHeight: 16px
    letterSpacing: 0.02em
  code-md:
    fontFamily: JetBrains Mono
    fontSize: 13px
    fontWeight: '500'
    lineHeight: 18px
  code-sm:
    fontFamily: JetBrains Mono
    fontSize: 11px
    fontWeight: '500'
    lineHeight: 16px
    letterSpacing: 0.04em
rounded:
  sm: 0.25rem
  DEFAULT: 0.5rem
  md: 0.75rem
  lg: 1rem
  xl: 1.5rem
  full: 9999px
spacing:
  gutter: 1rem
  margin: 2rem
  space-xs: 0.25rem
  space-sm: 0.5rem
  space-md: 1rem
  space-lg: 1.5rem
  space-xl: 2rem
---

## Brand & Style

The design system embodies a high-precision, hyper-focused desktop environment tailored for enthusiasts of ACG (Anime, Comic, Games) culture who demand peak client performance and absolute operational transparency. Built upon an uncompromising dark visual philosophy, the interface rejects muddy gray-blue undertones in favor of pure optical black (`#050505`) and neutral, zero-hue tiered surfaces. 

The aesthetic is grounded in **Technical Minimalism** blended with **Subtle Neon Futurism**:
- **Absolute Clarity:** Eliminates illustrative noise, ambient clutter, and visual debt. Every pixel serves a structural or navigational function.
- **Controlled Luminescence:** Neon energy is treated as a scarce visual resource. Glow effects are strictly rationed to active runtime states and primary launch triggers, eliminating visual fatigue.
- **Total Transparency:** Operates like a precision instrument panel. System changes, telemetry, and modding tiers (L1 configuration tweaks vs. L3 runtime hooks) are communicated with technical honesty and non-coercive UI semantics.

The emotional atmosphere is clinical, calm, focused, and instantly responsive, ensuring the user moves frictionlessly from launchpad to runtime immersion.

## Colors

The chromatic architecture is strictly regulated across three tiers: zero-hue foundations, high-contrast monochrome typography, and razor-sharp dual neon accents.

### Surface System
- **Base Canvas (`#050505`):** Complete ink-black foundation. Prevents gray washout on OLED and high-contrast IPS panels.
- **Zero-Hue Tiers:** Surface tiers rely exclusively on luminance without chroma shifts:
  - `surface-1` (`#0D0D0D`): Primary resting level for cards, container panels, and overlays.
  - `surface-2` (`#141414`): Nested groupings, secondary modules, and surface hover states.
  - `surface-3` (`#1E1E1E`): Input controls, progress tracks, and code block containers.
- **Hairlines (`rgba(255,255,255,0.08)`): Crisp 1px separators defining structural geometry without visually segmenting the canvas.

### Accent & Functional Semantics
- **Electric Cyan (`#00E5FF`):** The primary kinetic trigger. Used for positive verification (`Verified`, `Compatible`), active game execution (`Running`), and primary launch triggers.
- **Cyber Fuchsia (`#C77DFF`):** The contextual utility anchor. Dedicated to tools, enhancement plugins, pending attention strips, and modification boundaries (L1/L3 risk identifiers).
- **Critical Danger (`#FF4757`):** Reserved strictly for terminal destruction, incompatible dependencies, broken runtimes, and critical confirmation thresholds.
- **Typography:** Pure white (`#FFFFFF`) for primary headings, metric values, and interactive labels. 60% translucent white (`rgba(255,255,255,0.60)`) handles secondary meta-labels. Mid-gray body copy is strictly prohibited.

## Typography

Typography prioritizes technical legibility and international alignment. The font stack unites **Inter** for Latin numerals and English UI strings, **PingFang SC / MiSans** as the primary CJK fallback for Simplified Chinese interfaces, and **JetBrains Mono** for developer-oriented metadata, file hashes, timestamps, and log entries.

### Type Scale Rules
- **Headline-LG (20px / 28px):** The maximum visual anchor within standard window bounds (1280×800 desktop canvas). Used for page titles and hero game headers.
- **Headline-MD (16px / 24px):** Used for panel cards, drawer heads, and major module dividers.
- **Body-MD (14px / 20px):** Default standard for descriptive disclosures, dialog text, and table rows.
- **Body-SM & Label-SM (12px / 16px):** Used for secondary technical attributes (playtime counters, regions, semantic tags).
- **Code Series (13px & 11px):** Applied exclusively via `JetBrains Mono` to manifest file checksums (e.g. `a3f9c2`), system paths, and version tags.

## Layout & Spacing

The design system operates on a rigorous **8px geometric module**. Built specifically for desktop clients centered around a baseline 1280×800 footprint (scalable up to 4K displays), the spatial strategy emphasizes deliberate restraint and generous breathing room.

### Rhythm & Grid
- **Window Boundaries (`margin` = 32px / 2rem):** Outer frame safe zones retain a consistent 32px breathing boundary around content clusters.
- **Card Gaps (`gutter` = 16px / 1rem):** Between grid cards and stacked modules, a uniform 16px gap maintains architectural clarity.
- **Component Padding:**
  - Card containers use `space-lg` (24px) for expansive visual relief.
  - Interactive rows and compact cells use `space-md` (16px) or `space-sm` (8px).
- **Desktop Adaptation:**
  - At the baseline 1280×800 resolution, game libraries adopt a structured 3-to-4 column layout.
  - Multi-column expansion occurs smoothly beyond 1600px without expanding line lengths past optimal readability thresholds.

## Elevation & Depth

Visual hierarchy is constructed entirely through **luminance stepping**, **hairline edge definition**, and **rationed luminescence**. Drop shadows and simulated depth are systematically avoided in favor of crisp planar layering.

### Surface Elevation Tiers
1. **Tier 0 (Canvas):** Base window level (`#050505`).
2. **Tier 1 (Panels & Cards):** Raised planar level (`#0D0D0D`) bounded by an ultra-thin 1px border (`rgba(255, 255, 255, 0.08)`).
3. **Tier 2 (Nested Items & Hover States):** Interactive children, list items, or focused states resting at `#141414`.
4. **Tier 3 (Floating Modals & Overlays):** Elevated dialogs resting on `#141414` over a 70% black backdrop blur (`backdrop-filter: blur(12px)`).

### Rationed Luminescence Rules
Glow is functional, never decorative. It operates exclusively under two conditions:
- **Primary Execution CTA:** A diffuse outer neon cyan aura (`0 0 24px rgba(0, 229, 255, 0.35)`) on the primary launch button to establish the single dominant focal point.
- **Active Runtime Beacon:** A pulsating 8px cyan indicator dot with `0 0 12px rgba(0, 229, 255, 0.50)` signaling that a game or thread is executing.
All background cards, enhancement tiles, and utility panels must remain completely matte.

## Shapes

Form geometry mirrors the precision of industrial software with softened corners to provide tactile comfort.

- **Cards & Primary Modules:** 12px border radius. This geometry balances modern software conventions with high-density informational structure.
- **Buttons & Control Inputs:** 8px border radius. Provides a distinct click signature separate from the structural 12px cards.
- **Badges, Tags, & Status Pills:** Full capsule radius (`9999px`) to immediately isolate semantic indicators from clickable rectangular cards.
- **Cover Visuals:** Game art assets inherit standard 12px rounding, strictly masked within a 16:9 aspect ratio.

## Components

### Buttons
- **Primary Launch CTA:** Minimum height 48px. Solid fill `#00E5FF` with `#050505` bold typography. Features the signature `0 0 24px rgba(0, 229, 255, 0.35)` cyan glow. Hover increases brightness; active reduces scale to 0.98.
- **Fuchsia Utility Action:** Solid or bordered `#C77DFF` with pure white or `#050505` text, used specifically for enabling modifications and utility flows. Optional faint glow (`0 0 16px rgba(199, 125, 255, 0.30)`).
- **Ghost / Secondary Buttons:** Transparent background with hairline border `rgba(255,255,255,0.08)`, transitioning to `rgba(255,255,255,0.16)` on hover. Text `#FFFFFF`.

### Cards & Panels
- **Game Cards (16:9):** Background `#0D0D0D`, 1px hairline border, 12px corner radius. Features cover art, game title in pure white, region tag, playtime metrics in 60% white, and a corner status capsule.
- **Enhancement Cards:** Dedicated utility blocks featuring a 3px accent indicator line on the left edge (`#C77DFF`), clearly grouping tool permissions and toggle levers.

### Status Pills & Risk Badges
- **Verified / Running:** Capsule shape. Cyan border or light cyan fill (`rgba(0, 229, 255, 0.12)`) with `#00E5FF` text and inline checkmark or pulsing dot.
- **Risk Level Tags (L1 vs. L3):**
  - *L1 (Config Modification):* `#C77DFF` hairline outline with `#C77DFF` text.
  - *L3 (Process Hook / Kernel):* `#C77DFF` hairline outline accompanied by a prominent `#FF4757` warning dot. Defaults to disabled states.
- **Critical Error / Deprecated:** `#FF4757` outline or solid pill indicator.

### Input Fields & Controls
- **Form Inputs:** 40px height, `#1E1E1E` fill, hairline border, 8px radius. Active focus switches border to `#00E5FF` with no spread blur.
- **Checkboxes:** 18px square, `#141414` fill, hairline border. Checked state fills with `#C77DFF` (for consent flows) or `#00E5FF` (for generic preferences) with a sharp white checkmark.

### Step Execution Panel (Transparent Action Stream)
- Dedicated sequential flow container featuring discrete step items:
  1. `Completed`: Monospace timestamp, hash thumb (`a3f9c2`), cyan checkmark.
  2. `In Progress`: Micro animated progress bar in pure cyan (`#00E5FF`).
  3. `Pending`: 30% translucent disabled state (`rgba(255, 255, 255, 0.30)`).
  4. `Rollback State`: Automatic transition to `#FF4757` fail badge followed by an immediate cyan check confirmation of restored baseline files.