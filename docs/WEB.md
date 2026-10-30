# UniGIS in the browser (Netlify)

The app compiles to WebAssembly (`app/apps/rustgis-web`, eframe + WebGPU/WebGL2, built with
[Trunk](https://trunkrs.dev)) and includes UniGIS's ArcGIS-verified tools, so everything you can
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

## Without tokens: import the repository

Every push to `main` also force-pushes the finished site to the `web-dist` branch. In Netlify use
Add new site › Import an existing project › GitHub › `geoforge`, set **Branch to deploy** to
`web-dist`, leave the build command empty and the publish directory as `.`. Netlify then only
hosts the files (the branch carries its own `netlify.toml`); each push to `main` redeploys it
once the build above finishes. The token and secret steps are then not needed.

## From this machine (fallback)

The repository is public, so GitHub Actions builds and publishes on every push to `main` at no
cost. The local publisher below is only needed if Actions is unavailable.

`scripts/publish-web-dist.sh` builds `origin/main` in its own worktree (`~/.cache/unigis-web`) and
force-pushes the result to `web-dist`, which Netlify deploys. It does nothing if `web-dist` already
holds that commit's build. To keep memory free (about 8 GB) it holds new coordinator turns, waits
for the current one and any other Cargo build, requires at least **9 GiB available RAM**
(including 1 GiB headroom above the build estimate), and builds with **one Cargo job**. A file
lock prevents simultaneous publisher instances. The RAM check includes finite cgroup-v2
limits and runs again immediately before Trunk starts; low RAM makes the publisher wait
(up to two hours), rather than starting another large build. A user timer runs it every
4 hours:

```sh
cp tools/agent-coordinator/unigis-web-dist.{service,timer} ~/.config/systemd/user/
systemctl --user daemon-reload && systemctl --user enable --now unigis-web-dist.timer
scripts/publish-web-dist.sh --check-resources # read-only RAM check; does not publish
systemctl --user start unigis-web-dist      # publish now; journalctl --user -u unigis-web-dist -f
```

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

Maps, symbology, tables, layouts, the Tools pane and the UniGIS tools that do not need the file
system (shapefiles and GeoPackages are opened from the file picker). File geodatabase browsing and
the `ug` command line are desktop only. The build uses the `web` Cargo profile (single codegen unit,
fat LTO), so expect 15 to 40 minutes and about 8 GB of memory to compile it.
