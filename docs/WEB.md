# GeoForge in the browser (Netlify)

The app compiles to WebAssembly (`app/apps/rustgis-web`, eframe + WebGPU/WebGL2, built with
[Trunk](https://trunkrs.dev)) and includes GeoForge's ArcGIS-verified tools, so everything you can
click on the desktop can be tried from a URL. Data stays in the browser tab.

## One-time setup

1. Create a Netlify site (Add new site › Deploy manually, or `npx netlify-cli sites:create`). Note its
   **Site ID** (Site configuration › Site details).
2. Create a personal access token (User settings › Applications › Personal access tokens).
3. In the GitHub repository add the secrets `NETLIFY_AUTH_TOKEN` and `NETLIFY_SITE_ID`
   (Settings › Secrets and variables › Actions).

After that every push to `main` runs `.github/workflows/web-netlify.yml`, which builds the site and
publishes it to production. (Netlify's own build image cannot take this build: whole-program LTO of
the WebAssembly needs several GB of memory and a long compile, so it is built in CI and Netlify
only hosts the finished files; `netlify.toml` carries the headers.)

## By hand

```sh
rustup target add wasm32-unknown-unknown
# Trunk 0.21: https://github.com/trunk-rs/trunk/releases (or `cargo install trunk --locked`)
scripts/build-web.sh                       # writes app/dist/web
(cd app/dist/web && python3 -m http.server 8080)   # try it locally
NETLIFY_AUTH_TOKEN=... NETLIFY_SITE_ID=... scripts/deploy-netlify.sh          # draft deploy URL
NETLIFY_AUTH_TOKEN=... NETLIFY_SITE_ID=... scripts/deploy-netlify.sh --prod   # production
```

## What works in the browser

Maps, symbology, tables, layouts, the Tools pane and the GeoForge tools that do not need the file
system (shapefiles and GeoPackages are opened from the file picker). File geodatabase browsing and
the `gf` command line are desktop only. The build uses the `web` Cargo profile (single codegen unit,
fat LTO), so expect 15 to 40 minutes and about 8 GB of memory to compile it.
