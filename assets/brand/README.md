# Conduit brand assets

The Conduit mark is **Lanes**: three parallel routes bending around one centre into a C. The palette is **Ember**.

## Files

| File | Use |
|---|---|
| `conduit-logo-horizontal.svg` | Default logo on light backgrounds |
| `conduit-logo-horizontal-reverse.svg` | Default logo on dark backgrounds |
| `conduit-logo-stacked.svg` / `-reverse.svg` | Square-ish spaces (splash, avatars with room) |
| `conduit-logo-*-mono.svg` | One colour; inherits `currentColor` |
| `conduit-logo-*-ember.svg`, `conduit-mark-ember.svg` | All Ember (`#F25C1F`); readable on both light and dark grounds, for places where the background is unknown, such as README previews |
| `conduit-mark.svg` / `-reverse.svg` | Mark alone, 24 px and up |
| `conduit-mark-mono.svg` | Mark alone, one colour (`currentColor`) |
| `conduit-mark-small.svg` / `-reverse.svg` | Two-lane cut for 24 px and below |
| `conduit-app-icon.svg` | App icon tile, 1024 master, 22% corner radius |
| `favicon.svg`, `favicon.ico` | Browser favicon (ICO holds 16, 32 and 48 px) |
| `png/` | Raster exports: app icon 1024/512, `apple-touch-icon.png` (180, full-bleed), favicons 16/32, mark 512, logo previews 1200 px wide |

The wordmark in every `conduit-logo-*` file is outlined (paths), so no font is needed to display it.

In the project README, the logo is a `<picture>` element. The dark source is the reverse logo, and the fallback `<img>` is the Ember logo. Viewers that ignore `<picture>` or misreport dark mode therefore still show a readable logo.

```html
<link rel="icon" href="/favicon.ico" sizes="any">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
```

## Colour

| Name | Hex | Role |
|---|---|---|
| Ink | `#1C1410` | Lanes, wordmark, dark grounds |
| Ember | `#F25C1F` | Inner lane on light grounds |
| Ember Bright | `#FF7A3D` | Inner lane on dark grounds |
| Ember Deep | `#C2410C` | Small text and links on light grounds |
| Cream | `#FFF6EE` | Lanes and text on dark grounds |
| Stone | `#6B5F57` | Secondary text |
| Paper | `#F7F3EF` | Light page ground |

Use Ember for the inner lane and small accents only. For text on light grounds use Ember Deep, because plain Ember is too light for small text.

## Type

- **Geist SemiBold 600**, tracking −5%, always lowercase: the wordmark and headings
- **Geist Regular 400**: interface and body text
- **Geist Mono 400**: CLI, config and logs

Geist is licensed under the SIL Open Font License; see https://github.com/vercel/geist-font.

## Rules

- Construction is a 100-unit grid: stroke 8u, lane gap 6u, radii 40/26/12u, and each lane's terminal steps in 10u.
- Keep 20u of clear space on every side of the mark.
- Minimum size: mark 24 px, horizontal logo 96 px wide. Below 24 px, use `conduit-mark-small`.
- Do not recolour individual outer lanes, rotate or flip the mark, change the stroke weight, or set the wordmark in capitals.
