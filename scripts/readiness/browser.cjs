// Tests the real built WASM app using its documented embedding protocol and UI.
// No privileged test API is added to the product. Network basemaps/plugins are
// deliberately unavailable: this is an offline local-data workflow.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const http = require('node:http');
const { parseArgs } = require('node:util');
const { createHash } = require('node:crypto');

// A zip archive of stored (uncompressed) entries: enough to carry a folder's files.
function storedZip(files) {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xEDB88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc32 = data => {
    let c = 0xFFFFFFFF;
    for (const b of data) c = crcTable[(c ^ b) & 0xFF] ^ (c >>> 8);
    return (c ^ 0xFFFFFFFF) >>> 0;
  };
  const parts = [], central = [];
  let offset = 0;
  for (const { name, data } of files) {
    const n = Buffer.from(name, 'utf8'), crc = crc32(data);
    const head = (sig, isCentral) => {
      const h = Buffer.alloc(isCentral ? 46 : 30);
      let i = 0;
      const u32 = v => { h.writeUInt32LE(v >>> 0, i); i += 4; }, u16 = v => { h.writeUInt16LE(v, i); i += 2; };
      u32(sig);
      if (isCentral) u16(20);
      u16(20); u16(0x0800); u16(0); u16(0); u16(0x21);
      u32(crc); u32(data.length); u32(data.length); u16(n.length); u16(0);
      if (isCentral) { u16(0); u16(0); u16(0); u32(0); u32(offset); }
      return h;
    };
    const local = head(0x04034b50, false);
    parts.push(local, n, data);
    central.push(head(0x02014b50, true), n);
    offset += local.length + n.length + data.length;
  }
  const dir = Buffer.concat(central), end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(files.length, 8); end.writeUInt16LE(files.length, 10);
  end.writeUInt32LE(dir.length, 12); end.writeUInt32LE(offset, 16);
  return Buffer.concat([...parts, dir, end]);
}

async function main() {
  const { values } = parseArgs({ options: {
    dist: { type: 'string' }, report: { type: 'string', default: 'browser-readiness.json' },
    chromium: { type: 'string' }, screenshot: { type: 'string', default: 'browser-readiness.png' },
  } });
  assert(values.dist, '--dist must point to the built web app');
  const root = path.resolve(values.dist);
  const report = { timestamp_utc: new Date().toISOString(),
    scope: 'Offline Chromium: import, Buffer via UI, export, project save/reopen, file geodatabase upload, folder picker, download',
    checks: [], external_requests_blocked: [], page_errors: [] };
  const files = await fs.readdir(root);
  const wasm = files.find(name => name.endsWith('_bg.wasm'));
  assert(wasm, 'Built WASM artifact missing');
  report.wasm_sha256 = createHash('sha256').update(await fs.readFile(path.join(root, wasm))).digest('hex');
  const server = http.createServer(async (req, res) => {
    try {
      const pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
      const file = path.resolve(root, '.' + (pathname === '/' ? '/index.html' : pathname));
      if (!file.startsWith(root + path.sep)) { res.writeHead(403).end(); return; }
      const mime = { '.wasm': 'application/wasm', '.js': 'text/javascript', '.html': 'text/html', '.json': 'application/json' };
      res.setHeader('Content-Type', mime[path.extname(file)] || 'application/octet-stream');
      res.end(await fs.readFile(file));
    } catch { res.writeHead(404).end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const origin = `http://127.0.0.1:${server.address().port}`;
  let browser;
  try {
    browser = await chromium.launch({ executablePath: values.chromium,
      args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
    report.chromium = browser.version();
    const page = await browser.newPage({ viewport: { width: 1400, height: 900 }, serviceWorkers: 'block' });
    page.on('pageerror', error => report.page_errors.push(error.message));
    await page.route('**/*', route => {
      const url = route.request().url();
      if (new URL(url).origin === origin) return route.continue();
      report.external_requests_blocked.push(url);
      return route.abort();
    });
    await page.addInitScript(() => {
      window.readinessMessages = [];
      window.addEventListener('message', event => {
        if (event.source === window && event.origin === location.origin) window.readinessMessages.push(event.data);
      });
    });
    const ready = async () => {
      await page.goto(`${origin}/?embed`);
      await page.waitForFunction(() => window.readinessMessages.some(m => m.rustgis === 'ready'), null, { timeout: 60000 });
    };
    let serial = 0;
    const command = async (command, params = {}) => {
      const id = ++serial;
      await page.evaluate(message => window.postMessage(message, location.origin), { rustgis: 'run', id, command, params });
      await page.waitForFunction(id => window.readinessMessages.some(m => m.rustgis === 'result' && m.id === id), id);
      const response = await page.evaluate(id => window.readinessMessages.find(m => m.rustgis === 'result' && m.id === id), id);
      assert.equal(response.ok, true, `${command}: ${response.error}`);
    };
    const projectWith = async name => {
      await page.waitForFunction(name => window.readinessMessages.some(m => m.rustgis === 'project' &&
        JSON.parse(m.text).maps.some(map => map.layers.some(layer => layer.name === name))), name);
      return page.evaluate(name => JSON.parse(window.readinessMessages.filter(m => m.rustgis === 'project').reverse()
        .find(m => JSON.parse(m.text).maps.some(map => map.layers.some(layer => layer.name === name))).text), name);
    };
    await ready();
    report.checks.push('wasm_startup');
    const source = { type: 'FeatureCollection', features: [
      { type: 'Feature', properties: { ID: 1 }, geometry: { type: 'Point', coordinates: [13, 52] } },
      { type: 'Feature', properties: { ID: 2 }, geometry: { type: 'Point', coordinates: [13.01, 52.01] } },
    ] };
    const picker = page.waitForEvent('filechooser');
    await command('ui.addData');
    await (await picker).setFiles({ name: 'readiness.geojson', mimeType: 'application/geo+json', buffer: Buffer.from(JSON.stringify(source)) });
    const imported = await projectWith('readiness');
    assert.equal(imported.maps[0].layers.find(l => l.name === 'readiness').dataset.features.length, 2);
    report.checks.push('import_geojson');
    await command('ui.gp.open', { tool: 'unigis:buffer', values: { input: 'readiness', distance: 100, output: 'buffered' } });
    // egui is a canvas. At this fixed viewport the Run button is at (1340,708).
    // Assert the resulting project/history, so UI changes cannot silently pass.
    await page.mouse.click(1340, 708);
    const buffered = await projectWith('buffered');
    const result = buffered.maps[0].layers.find(l => l.name === 'buffered');
    assert.equal(result.dataset.features.length, 2);
    assert.equal(result.dataset.geometry_type, 'Polygon');
    assert(buffered.gp_history.length > 0, 'Tool execution missing from history');
    report.checks.push('buffer_two_points');
    await fs.mkdir(path.dirname(path.resolve(values.screenshot)), { recursive: true });
    await page.screenshot({ path: values.screenshot });
    // Geoprocessing retains the input selection. Select the second Contents row
    // explicitly before exporting, as a user must do.
    await page.mouse.click(110, 312);
    const exporting = page.waitForEvent('download');
    await command('ui.exportLayer', { format: 'geojson' });
    const exported = await exporting;
    assert.equal(exported.suggestedFilename(), 'buffered.geojson');
    const geojson = JSON.parse(await fs.readFile(await exported.path(), 'utf8'));
    assert.equal(geojson.features.length, 2);
    assert.deepEqual(geojson.features.map(f => f.properties.ID).sort(), [1, 2]);
    for (const feature of geojson.features) {
      assert(['Polygon', 'MultiPolygon'].includes(feature.geometry.type));
      assert.equal(feature.properties.BUFF_DIST, 100);
      const center = source.features.find(f => f.properties.ID === feature.properties.ID).geometry.coordinates;
      const ring = feature.geometry.type === 'Polygon' ? feature.geometry.coordinates[0] : feature.geometry.coordinates[0][0];
      assert(ring.length >= 4, 'Buffer ring is empty or degenerate');
      for (const point of ring) {
        const rad = Math.PI / 180;
        const dlat = (point[1] - center[1]) * rad, dlon = (point[0] - center[0]) * rad;
        const a = Math.sin(dlat / 2) ** 2 + Math.cos(center[1] * rad) * Math.cos(point[1] * rad) * Math.sin(dlon / 2) ** 2;
        const metres = 6371008.8 * 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
        assert(Number.isFinite(metres) && Math.abs(metres - 100) < 2,
          `Buffer vertex is ${metres} m from its input (expected approximately 100 m)`);
      }
    }
    report.checks.push('export_buffer_geojson');
    const saving = page.waitForEvent('download');
    await command('ui.saveAs');
    await page.mouse.click(878, 462); // Save Project As confirmation at the fixed viewport.
    const saved = await saving;
    const bytes = await fs.readFile(await saved.path());
    const savedProject = JSON.parse(bytes);
    assert.equal(savedProject.maps[0].layers.length, 2);
    await fs.mkdir(path.dirname(path.resolve(values.screenshot)), { recursive: true });
    await page.screenshot({ path: values.screenshot });
    await ready();
    const reopening = page.waitForEvent('filechooser');
    await command('ui.open');
    await (await reopening).setFiles({ name: 'readiness.rgis', mimeType: 'application/json', buffer: bytes });
    const reopened = await projectWith('buffered');
    assert.deepEqual(reopened.maps[0].layers.map(l => l.dataset), savedProject.maps[0].layers.map(l => l.dataset));
    report.checks.push('save_reload_reopen_project');
    // A file geodatabase, as a zip of its folder (a .gdb is a folder; the page takes files).
    // It is listed in the Catalog under Databases and its feature class is added as a layer.
    const gdbDir = path.resolve(__dirname, '../../testdata/gdb-reference/edit_fc.gdb');
    const gdbFiles = [];
    for (const name of (await fs.readdir(gdbDir)).sort()) {
      gdbFiles.push({ name: `edit_fc.gdb/${name}`, data: await fs.readFile(path.join(gdbDir, name)) });
    }
    const gdbPicker = page.waitForEvent('filechooser');
    await command('ui.addData');
    await (await gdbPicker).setFiles({ name: 'edit_fc.zip', mimeType: 'application/zip', buffer: storedZip(gdbFiles) });
    const withGdb = await projectWith('wells');
    const wells = withGdb.maps[0].layers.find(l => l.name === 'wells');
    assert(wells.dataset.features.length > 0, 'Geodatabase feature class added without features');
    assert.equal(wells.dataset.source, 'edit_fc.gdb');
    report.checks.push('add_file_geodatabase_zip');
    // Add Geodatabase…: the browser's folder picker on a .gdb folder connects it to the project
    // (Catalog › Databases) without adding layers, as ArcGIS's Add Database does.
    const layersBefore = withGdb.maps[0].layers.length;
    const folderPicker = page.waitForEvent('filechooser');
    await command('ui.addGeodatabase');
    await (await folderPicker).setFiles(path.resolve(__dirname, '../../testdata/gdb-reference/domain_base.gdb'));
    await page.waitForFunction(() => window.readinessMessages.some(m => m.rustgis === 'project' &&
      (JSON.parse(m.text).databases || []).some(d => d.name === 'domain_base.gdb')), null, { timeout: 60000 });
    const connected = await page.evaluate(() => JSON.parse(window.readinessMessages.filter(m => m.rustgis === 'project').pop().text));
    assert.equal(connected.maps[0].layers.length, layersBefore, 'Add Geodatabase added layers');
    assert.deepEqual(connected.databases.map(d => d.name).sort(), ['domain_base.gdb', 'edit_fc.gdb']);
    report.checks.push('add_geodatabase_folder_picker');
    // Download hands an upload back as a zip of its folder.
    const downloading = page.waitForEvent('download');
    await command('ui.downloadGeodatabase', { geodatabase: 'domain_base.gdb' });
    const zip = await fs.readFile(await (await downloading).path());
    assert.equal(zip.readUInt32LE(0), 0x04034b50, 'not a zip archive');
    assert(zip.includes(Buffer.from('domain_base.gdb/a00000001.gdbtable')), 'the geodatabase folder is missing');
    report.checks.push('download_geodatabase');
    assert.deepEqual(report.page_errors, [], 'Browser runtime errors');
    report.passed = true;
  } catch (error) {
    report.passed = false;
    report.error = error.stack;
  } finally {
    if (browser) await browser.close();
    await new Promise(resolve => server.close(resolve));
    report.external_requests_blocked = [...new Set(report.external_requests_blocked)];
    await fs.mkdir(path.dirname(path.resolve(values.report)), { recursive: true });
    await fs.writeFile(values.report, JSON.stringify(report, null, 2) + '\n');
  }
  console.log(JSON.stringify(report, null, 2));
  process.exitCode = report.passed ? 0 : 1;
}
main().catch(error => { console.error(error); process.exitCode = 1; });
