---
name: Occult-Tech Hermetica
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
  on-surface-variant: '#d0c5af'
  inverse-surface: '#e5e2e1'
  inverse-on-surface: '#313030'
  outline: '#99907c'
  outline-variant: '#4d4635'
  surface-tint: '#e9c349'
  primary: '#f2ca50'
  on-primary: '#3c2f00'
  primary-container: '#d4af37'
  on-primary-container: '#554300'
  inverse-primary: '#735c00'
  secondary: '#dac3aa'
  on-secondary: '#3c2e1c'
  secondary-container: '#574633'
  on-secondary-container: '#ccb59c'
  tertiary: '#bfcdff'
  on-tertiary: '#082b72'
  tertiary-container: '#97b0ff'
  on-tertiary-container: '#254188'
  error: '#ffb4ab'
  on-error: '#690005'
  error-container: '#93000a'
  on-error-container: '#ffdad6'
  primary-fixed: '#ffe088'
  primary-fixed-dim: '#e9c349'
  on-primary-fixed: '#241a00'
  on-primary-fixed-variant: '#574500'
  secondary-fixed: '#f7dec4'
  secondary-fixed-dim: '#dac3aa'
  on-secondary-fixed: '#261909'
  on-secondary-fixed-variant: '#544431'
  tertiary-fixed: '#dbe1ff'
  tertiary-fixed-dim: '#b4c5ff'
  on-tertiary-fixed: '#00174b'
  on-tertiary-fixed-variant: '#27438a'
  background: '#131313'
  on-background: '#e5e2e1'
  surface-variant: '#353534'
  bone-white: '#F5F5F5'
  parchment: '#E8E2D2'
  deep-void: '#050505'
  sigil-blue: '#0000F2'
typography:
  display-xl:
    fontFamily: Playfair Display
    fontSize: 64px
    fontWeight: '700'
    lineHeight: '1.1'
    letterSpacing: -0.02em
  headline-lg:
    fontFamily: Playfair Display
    fontSize: 32px
    fontWeight: '600'
    lineHeight: '1.2'
  headline-lg-mobile:
    fontFamily: Playfair Display
    fontSize: 24px
    fontWeight: '600'
    lineHeight: '1.2'
  body-md:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: '400'
    lineHeight: '1.6'
  label-mono:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: '500'
    lineHeight: '1.4'
    letterSpacing: 0.1em
  label-mono-bold:
    fontFamily: JetBrains Mono
    fontSize: 12px
    fontWeight: '700'
    lineHeight: '1.4'
spacing:
  unit: 4px
  container-max-width: 1100px
  gutter: 24px
  margin-mobile: 16px
  margin-desktop: 64px
---

## Brand & Style

The design system embodies the "Occult-Tech" aesthetic—a synthesis of Renaissance alchemy and futuristic terminal interfaces. It targets an audience that values deep technical capability paired with a high-art, philosophical narrative. 

The visual style is **Minimalist-Gothic**. It utilizes heavy whitespace (or "darkspace"), high-contrast typography, and textures that evoke both ancient parchment and digital grain. The emotional response is one of reverence, mystery, and concentrated power. The UI should feel less like a modern SaaS application and more like an unearthed digital artifact or a forbidden ritualistic interface.

## Colors

The palette is anchored in **Deep Void (#050505)** to provide an infinite, atmospheric background. **Bone White** and **Parchment** are used for primary text and iconography to evoke the feeling of ink on aged paper.

**Metallic Gold (#D4AF37)** serves as the primary accent color, used sparingly for critical interactive states and decorative sigils. **Sigil Blue** is reserved for high-intensity system alerts or "glitch" moments where the technology pierces the occult facade. Dark browns are utilized for container backgrounds and subtle depth shading, ensuring the UI never feels purely monochromatic.

## Typography

This design system relies on a sharp contrast between **Playfair Display**, an elegant and high-contrast serif for headings, and **JetBrains Mono** for technical UI elements. This juxtaposition represents the "Ancient Manuscript" meeting the "Terminal."

- **Headlines:** Should be treated as editorial elements. Use wide tracking for a more "monumental" feel in subheadings.
- **Body:** **Inter** provides high legibility for long-form textual output, ensuring the "Agent" communications remain readable.
- **Labels:** All interactive metadata, button labels, and system status indicators must use **JetBrains Mono** in uppercase to reinforce the technical nature of the interface.

## Layout & Spacing

The layout philosophy follows a **Fixed Grid** model with a strong emphasis on a centered vertical axis. Content is rarely spread across the full width of the screen, instead inhabiting a focused central column that evokes the proportions of a book or a scroll.

- **Desktop:** A 12-column grid with wide outer margins (64px+) to create a sense of isolation and focus.
- **Mobile:** A fluid single-column layout with 16px margins. 
- **Rhythm:** Use a 4px baseline grid. Spacing between sections should be generous (typically 80px or 120px) to allow the background imagery and textures to "breathe."

## Elevation & Depth

Depth is conveyed through **Tonal Layers** rather than traditional shadows. Surfaces do not "float"; they are etched or layered.

- **Backgrounds:** Incorporate a subtle noise/film grain overlay (3-5% opacity) and low-contrast classical imagery (engravings, star maps) in the furthest background layer.
- **Surfaces:** Containers use a semi-transparent dark brown or grey (`rgba(10, 10, 10, 0.8)`) with a `backdrop-filter: blur(10px)` to create a "Smoked Glass" effect.
- **Outlines:** Instead of shadows, use 1px "Ghost Borders" in a low-opacity Bone White or Gold. This creates a sharp, technical definition without the "softness" of modern SaaS designs.

## Shapes

The shape language is strictly **Sharp (0px)**. Rounded corners are avoided to maintain a rigorous, architectural, and slightly aggressive aesthetic. 

Small exceptions may be made for purely circular elements (like profile avatars or status pips) to represent celestial bodies or alchemical symbols, but all structural UI components—buttons, inputs, cards—must remain rectangular.

## Components

- **Buttons:** Rectangular with a 1px border. Default state is a ghost button (transparent fill, Bone White border). Hover state fills the button with a subtle gold tint and switches text to black.
- **Input Fields:** Bottom-border only, resembling a line on a ledger. The cursor should be a block-style monospaced underscore.
- **Chips/Tags:** Monospaced text enclosed in brackets (e.g., `[SYSTEM_ACTIVE]`) rather than solid pills.
- **Cards:** Used sparingly. They should appear as thin-lined boxes with a header title in a serif font, separated by a horizontal rule from the monospaced content below.
- **Progress Bars:** Represented as "loading sigils" or a simple thin line that fills with the accent Gold color.
- **Scrollbars:** Custom-styled to be ultra-thin (2px) and Bone White, appearing only on hover to minimize visual clutter.