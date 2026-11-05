# Website

Code: `site/`, `.github/workflows/pages.yml`

The landing page for this window, asked for on 2026-11-05 ("ultra mega impactante, full motion, only a landing", as cornercase has at `usecornercase.dev`). One page, in English, no docs: the docs stay on `usecornercase.dev`.

## Stack and hosting

- **Astro, as cornercase's own site**, so both repositories build alike, with no framework on the client. The animation has **no runtime dependency**: GSAP's licence is not compatible with this project's rule on licences (it is a no-charge licence with restrictions, not an open one), and what the page needs (a value per frame from the scroll) is small. `lib/` holds the plain logic, tested without a browser (`npm test`, `node:test` bundled with esbuild); `client/` the DOM and WebGL.
- **Every URL is relative** (`assetsPrefix: '.'`, stylesheets inlined, so a font's `url()` is relative to the page and not to a CSS file). The same build works at `usecornercase.github.io/cornercase-gpui/` and at `gui.usecornercase.dev/`, so changing the domain needs no rebuild. cornercase's site uses an absolute `SITE_BASE`, which breaks the assets until the next build after a domain change. Only the canonical and `og:` addresses take `SITE_ORIGIN` and `SITE_BASE` from `actions/configure-pages`.
- **The domain** is `gui.usecornercase.dev`: a `CNAME` from `gui` to `usecornercase.github.io` at DigitalOcean, where `usecornercase.dev` is (cornercase's `design/website.md`), and the custom domain in this repository's Settings › Pages. A deployment from Actions needs no `CNAME` file. GitHub redirects the github.io address once it is set.
- **`og.png`** is a shot of the page's hero at 1200×630, made by hand with a headless Chrome: retake it when the hero's words change.

## The field

- **One fluid simulation behind the whole page, drawn in terminal cells** (`client/field/`, WebGL2): Navier-Stokes on a small grid (about 90 rows of velocity, one dye texel per cell), and a full-screen pass that picks a glyph per cell from an atlas drawn with JetBrains Mono, by the dye's density or, where it runs fast, by its direction (`─ ╱ │ ╲`). The product is a terminal drawn on the GPU, so the page's ground is too. Rejected: a video, which does not answer the pointer; a noise shader alone, which has no currents to push.
- **Five walkers keep it alive without a pointer**, the pointer pushes it, the scroll's speed drifts it, and a status changes its colour in Know who needs you, with a burst at each beat. Colours are the app's: cobalt, and yellow, orange, red and green only for working, waiting, failed and done.
- **The mark is drawn in cells by the same pass** (`markOn`, its 12-unit grid in the shader): it assembles at load, is stamped into the dye and blown away, and assembles again above the final title. Cells fray at random, so it reads as printed by a terminal.
- **It can give up**: at 38 fps or less for 1.5 s it lowers the pressure iterations, then drops to one device pixel and removes the window's backdrop blur (`.lite`), the costliest thing on the page. No WebGL2 or no renderable float texture falls back to the still page.

## The window

- **Drawn in HTML at the app's real size** (`components/stage/`, 1280×800, the app's default), from `src/theme.rs`'s colours, `assets/icons/` and the measures of a real capture, then moved as one object by a camera. Vector text stays sharp at any zoom, and glass shows the live field through it, which a picture could not do. It is an illustration, not a simulation: the terminal's own site simulates its app, at the cost of a rule to keep them in step, and this page shows one scripted story. The cast is the README screenshot's and cornercase's site's (web-shop, dark mode, gift cards, the empty address), so all the pictures tell one session.
- **A camera, not a layout, makes it responsive** (`lib/track.ts`): each shot names a rectangle of the window and the side left free for the caption, and the pose is worked out for the viewport. On a phone the same window is framed tighter above a caption sheet, so the text in it stays readable instead of a 390 px wide miniature.
- **Everything is a function of the scroll**, smoothed once (`client/scroll.ts`), and writes are cached (`client/paint.ts`), so scrolling back, a jump to an anchor and a reload midway need no special code. Native scrolling stays: the page does not hijack the wheel. Short state changes (a toast, a dock page) are CSS transitions set by a class, at the app's own 180 ms in, 120 ms out.
- **What stays true to the app**: labels, keys, statuses, the Browser's picked card and its "Reloaded: claude finished", the Flow panel's plan, and the panel widths. **What is simplified**: panes that do not fit beside the dock stack instead of shrinking; the conversation's look follows Claude Code as cornercase's simulated agent does.

## Real captures

- **The sheets' ring shows real screens, not drawings** (`src/assets/sheets/`), and so do the still pictures (`src/assets/stills/`): the debug build on a private server, with cornercase's `site/capture/` fake home (its projects, agents, `gh`), `CORNERCASE_GPUI_OPEN=<panel>`, a dark window behind the app for its blur to show, and `screencapture -R` of the window's rectangle, since `-l` composes the window over grey without its blur. They were taken on 2026-11-05 with cornercase 0.13.0; retake them when a sheet changes much. Paths in them are the fake home's.
- **Why a ring you stand inside**: eleven sheets one key apart read as one room, and the neighbours wrapping around the current one say there are more. A grid would be a wall of small screens, and it is what the page falls back to without motion.

## Without motion

- **The plain page comes first**: without JavaScript, with reduced motion, without WebGL2, or if the script has not run after 4 s, the sections flow as ordinary text with real pictures (`html:not(.motion)`). The inline script adds `.motion` before the first paint, so the show has no flash of the plain page. The window and the field are `aria-hidden`; the captions and headings are the page's text.
- **`.motion` keeps the semantic order**: titles are split in lines and letters only inside `aria-hidden` spans, with the whole text as the heading's `aria-label`.

## What the page claims

- Keep claims to what the README and these notes back: GPUI, the platforms and their requirements, signing, updates. The download's size and version come from GitHub's API at load, and the page says nothing of them if that fails. Nothing is compared with another app, and no frame rate is promised: the counter in the corner measures the visitor's own page.
- A visitor on Linux or Windows reads the keys as they are there (`lib/os.ts`, the README's rules).
- `tests/copy.test.ts` checks the sheets' keys against `src/keymap.rs`, their names against `src/panels/mod.rs` and the downloads against the README.

## Checked

Checked live on 2026-11-05 on an M5, headless, with Playwright: Chrome (GPU through ANGLE Metal) at 1440×900 and 390×844, every section at several scroll positions, the intro frame by frame, the built page served under `/cornercase-gpui/`, a reload midway, the anchors, reduced motion and no JavaScript; a whole scroll with the wheel held 60 fps, the most headless Chrome draws, with a worst frame of 33 ms. WebKit and Firefox at 1440×900, every section: WebKit ignored a perspective set two levels up and drew the ring from outside until each card took its own (`Sheets::placeCards`); headless Firefox ran slowly enough to fall back to `.lite`, which also proved the fallback. Not checked: Safari itself, a real phone, or a weak GPU.
