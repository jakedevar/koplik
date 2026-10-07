# #1652 visual polish: before and after (production build, real release data)

Screenshots: `before-<page>-<desktop|mobile>.png` and `after-<page>-<desktop|mobile>.png` for explorer, forecast, what-if and sources.
Desktop is 1280 px wide, mobile 390 px, full page, device scale factor 1, headless Chrome at `/opt/google/chrome/chrome`.
Both sets come from `vite build` (base `/koplik/`) of the same web data, `data/release` through `make pipeline-release` into
`web/public/data`, served by `web/scripts/static-server.mjs` (gzip, as GitHub Pages does) and captured by
`web/scripts/capture-ui.mjs` (`node scripts/capture-ui.mjs <dist> <out> <before|after> --shots --paint`). Before is origin/rolling at e123cfb; after is this branch.
Same page state in every shot: Explorer on Texas with the default measure, the forecast loaded, the what-if finished at baseline coverage.

**The Explorer map is not a reliable picture.** Headless Chrome here renders WebGL in software (SwiftShader). At 1280 px the map canvas
draws only a strip at the left (and at 390 px it draws fully); this is identical in the before and after shots and is not something this change touched.
Look at the rest of the page, not the map, for the desktop comparison. (The synthetic fixture map renders striped/blank for the same reason.)

## First paint (`before-paint.json`, `after-paint.json`)

Method: 7 fresh browser contexts per profile on the production build; the median of each is reported. FCP and LCP from `PerformanceObserver`
(`paint`, `largest-contentful-paint`); "content" is when the first headline number (`.headline-value button`) is in the DOM; CLS is layout shift.
Profile `unthrottled` is loopback on a 1280 px window; `mobile_throttled` is a 390 px window with CDP 4x CPU slowdown,
1.6 Mbit/s down and 150 ms round trip. Chrome picks LCP as the largest text painted, which before was the "Loading" notice, so "content" is the honest
"useful" time.

| profile | metric | before | after |
| --- | --- | --- | --- |
| unthrottled | FCP | 148 ms | 32 ms |
| unthrottled | LCP | 148 ms | 32 ms |
| unthrottled | content (first headline number) | 253 ms | 166 ms |
| mobile_throttled | FCP | 2644 ms | 436 ms |
| mobile_throttled | LCP | 4208 ms | 2244 ms |
| mobile_throttled | content (first headline number) | 4183 ms | 2219 ms |
| both | CLS | 0.000 | 0.000 |

Why: the shell and a skeleton are plain HTML in `index.html` (first paint no longer waits for JavaScript); MapLibre (285 kB gzip), the forecast
and the what-if engine (WASM, worker) load only when their page is first shown, so the first script is 72 kB gzip instead of 382 kB;
the Explorer's data files are preloaded in parallel with the script. No data was dropped or truncated.
