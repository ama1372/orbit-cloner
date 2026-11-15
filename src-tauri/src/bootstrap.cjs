// Runs before the vendor's main module, only inside a managed copy.
const fs = require('fs');
const path = require('path');
const Module = require('module');
const { pathToFileURL } = require('url');
const electron = require('electron');
const cfg = JSON.parse(fs.readFileSync(path.join(__dirname, 'profile.json'), 'utf8'));
const root = path.resolve(__dirname, '../../..');
const data = path.join(root, 'data');
fs.mkdirSync(data, { recursive: true });
const { app } = electron;
const archive = path.join(__dirname, '..', 'vendor.asar');
const icon = path.join(root, 'icon.png');
const log = (message) => fs.appendFileSync(path.join(root, 'runtime.log'), `${new Date().toISOString()} ${message}\n`);
log('bootstrap started');
const setPath = app.setPath.bind(app);
setPath('userData', data);
setPath('sessionData', data);
app.setPath = (key, value) => setPath(key, ['userData', 'sessionData'].includes(key) ? data : value);
app.setName(cfg.name);
app.setName = () => {};
app.getName = () => cfg.name;
app.setAppUserModelId(cfg.identity);
app.setAppUserModelId = () => {};
app.getAppPath = () => archive;
// Registration belongs to the manager. Never take over the installed app's handler.
app.setAsDefaultProtocolClient = () => false;
app.removeAsDefaultProtocolClient = () => false;
const originalLoad = Module._load;
class ProfileWindow extends electron.BrowserWindow {
  constructor(options = {}) { super({ ...options, icon }); }
}
class ProfileTray extends electron.Tray {
  constructor() { super(icon); this.setToolTip(cfg.name); }
  setImage() { return super.setImage(icon); }
  setToolTip() { return super.setToolTip(cfg.name); }
}
const wrapped = new Proxy(electron, { get(target, key) {
  if (key === 'BrowserWindow') return ProfileWindow;
  if (key === 'Tray') return ProfileTray;
  return target[key];
}});
Module._load = function(id, ...args) {
  return id === 'electron' || id === 'electron/main' ? wrapped : originalLoad.call(this, id, ...args);
};
// Apply identity even when the application imports Electron through ESM.
app.on('browser-window-created', (_, win) => {
  const setIcon = win.setIcon.bind(win);
  const setTitle = win.setTitle.bind(win);
  win.setIcon = () => setIcon(icon);
  win.setTitle = () => setTitle(cfg.name);
  win.setIcon();
  win.setTitle();
  win.on('page-title-updated', event => { event.preventDefault(); win.setTitle(cfg.name); });
});
// Managed copies must be refreshed through the manager, not vendor updaters.
for (const key of ['checkForUpdates', 'quitAndInstall', 'setFeedURL']) {
  if (electron.autoUpdater) electron.autoUpdater[key] = () => {};
}
const pkg = JSON.parse(fs.readFileSync(path.join(archive, 'package.json'), 'utf8'));
const entry = path.join(archive, pkg.main || 'index.js');
app.getVersion = () => pkg.version || '0.0.0';
// Set the application path back to the vendor archive for native module lookup.
process.argv = [process.argv[0], ...process.argv.slice(1).filter(x => !x.startsWith('--orbit-'))];
const fail = error => { log(error.stack || String(error)); electron.dialog.showErrorBox('Orbit Cloner', String(error)); app.exit(1); };
try {
  if (pkg.type === 'module' || entry.endsWith('.mjs')) import(pathToFileURL(entry).href).catch(fail);
  else { Module._load(entry, null, true); log('vendor main loaded'); }
} catch (error) { fail(error); }
