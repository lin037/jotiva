// Run after building the frontend. Keep the test profile, files and WebView data under target.
import { spawn, spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { once } from "node:events";
import { fileURLToPath, pathToFileURL } from "node:url";
import { setTimeout as delay } from "node:timers/promises";

const project = fileURLToPath(new URL("../", import.meta.url));
const native = path.join(project, "src-tauri");
const target = path.join(native, "target");
mkdirSync(target, {recursive: true});
const buildTarget = path.join(target, "desktop-qa-build");
const work = mkdtempSync(path.join(target, "windows-smoke-"));
const dist = path.join(work, "dist");
const data = path.join(work, "profile");
// The installed copy can legitimately own the default global shortcut.
mkdirSync(data, {recursive: true});
writeFileSync(path.join(data, "settings.json"), JSON.stringify({quick:"Control+Shift+F9", search:null, dock:null}));
writeFileSync(path.join(data, "appearance.json"), JSON.stringify({language:"en"}));
const hostTitle = "Jotiva QA " + process.pid;
const doc = path.join(work, "跨平台验收.txt");
const lines = Array.from({length: 350}, (_, i) => "第 " + (i + 1) + " 行 · Plain text");
const original = ["第一页", ...lines, "LAST-PAGE"].join("\r\n");
writeFileSync(doc, original);
const hostPage = path.join(work, "host.html");
writeFileSync(hostPage, "<!doctype html><title>" + hostTitle + "</title><p>" + hostTitle + "</p>");
cpSync(path.join(project, "dist"), dist, {recursive: true});
cpSync(new URL("./windows/probe.js", import.meta.url), path.join(dist, "windows-probe.js"));
cpSync(new URL("./desktop-probe.js", import.meta.url), path.join(dist, "probe.js"));
const page = path.join(dist, "index.html");
writeFileSync(page, readFileSync(page, "utf8").replace("</body>", '<script type="module" src="/probe.js?desktop-input=1&host=' + encodeURIComponent(hostTitle) + '"></script></body>'));
const config = {identifier: "io.github.lin037.jot.desktop.qa" + process.pid,
  build: {frontendDist: path.relative(native, dist).split(path.sep).join("/")}, app: {appDirectoriesOverride: data}};
const build = spawnSync("cargo", ["build", "--locked", "--target-dir", buildTarget, "--features", "tauri/custom-protocol"], {
  cwd: native, env: {...process.env, TAURI_CONFIG: JSON.stringify(config)}, stdio: "inherit"
});
if (build.status !== 0) throw Error("Unable to build Windows test application");
const executable = path.join(work, "Jotiva.exe");
cpSync(path.join(buildTarget, "debug", "Jotiva.exe"), executable);
const edge = [process.env["ProgramFiles(x86)"], process.env.ProgramFiles].filter(Boolean)
  .map(dir => path.join(dir, "Microsoft/Edge/Application/msedge.exe")).find(existsSync);
if (!edge) throw Error("Microsoft Edge is required for the external application drag test");
const host = spawn(edge, ["--user-data-dir=" + path.join(work, "edge-profile"),
  "--no-first-run", "--no-default-browser-check", "--disable-background-mode", "--disable-sync",
  "--app=" + pathToFileURL(hostPage).href], {windowsHide: true, stdio: "ignore"});
const app = spawn(executable, [doc], {windowsHide: true, stdio: "inherit"});
const completed = new Set();
let openedMenu = false;
let passed = false;
try {
  for (let attempt = 0; ; attempt++) {
    const notes = path.join(data, "notes");
    const records = existsSync(notes) ? readdirSync(notes).filter(f => /^\d+\.json$/.test(f)) : [];
    const results = records.flatMap(f => {
      try { return [JSON.parse(readFileSync(path.join(notes,f), "utf8")).markdown]; } catch { return []; }
    }).filter(Boolean);
    const error = results.find(s => s.startsWith("# QA error"));
    if (error) throw Error(error);
    for (const request of results.filter(s => s.startsWith("# QA input "))) {
      if (completed.has(request)) continue;
      const action = JSON.parse(request.slice("# QA input ".length));
      const input = spawnSync("powershell.exe", ["-NoProfile", "-STA", "-NonInteractive", "-File",
        fileURLToPath(new URL("./windows/input.ps1", import.meta.url))], {
        env: {...process.env, JOT_QA_INPUT: JSON.stringify(action), JOT_QA_PID: String(app.pid),
          JOT_QA_EDGE: String(host.pid), JOT_QA_HOST: hostTitle}, timeout: 12000, stdio: "inherit"
      });
      if (input.status !== 0) throw Error("Desktop input failed: " + action.name);
      completed.add(request);
      console.log("Desktop input:", action.name);
    }
    if (results.includes("# QA menu") && !openedMenu) {
      const selected = spawnSync("powershell.exe", ["-NoProfile", "-STA", "-NonInteractive", "-File",
        fileURLToPath(new URL("./windows/menu.ps1", import.meta.url))], {
        env: {...process.env, JOT_QA_PID: String(app.pid)}, stdio: "inherit", timeout: 10000
      });
      if (selected.status !== 0) throw Error("Unable to select native Tools > Settings");
      openedMenu = true;
    }
    if (results.includes("# QA attached") && host.exitCode === null && !host.killed) host.kill();
    if (["settings", "document", "main"].every(label => results.includes("# QA " + label))) break;
    if (app.exitCode !== null) throw Error("Application exited early: " + app.exitCode);
    if (attempt >= 480) throw Error("Timed out waiting for WebView checks " + JSON.stringify(results.filter(s => s.startsWith("# QA "))));
    await delay(250);
  }
  const selected = spawnSync("powershell.exe", ["-NoProfile", "-STA", "-NonInteractive", "-File",
    fileURLToPath(new URL("./windows/explorer.ps1", import.meta.url))], {env: {...process.env, JOT_QA_DOCUMENT: doc}, stdio: "inherit"});
  if (selected.status !== 0) throw Error("Explorer did not select the test document");
  const saved = readFileSync(doc, "utf8");
  const edits = ["Desktop edit", "quick-sync", "main-sync"];
  if (!edits.every(edit => saved.includes(edit)) || edits.reduce((body, edit) => body.replace(edit, ""), saved) !== original) {
    throw Error("Text or original CRLF line endings did not survive the round trip");
  }
  passed = true;
  console.log("Windows WebView2 passed plain text and CRLF preservation, native keyboard input, cross-window drag to Edge, mounted editor sync and caption controls, global capture from Edge, host closure, Explorer attachment and file selection.");
} finally {
  if (app.exitCode === null) {
    const stopped = once(app, "exit");
    app.kill();
    await stopped;
  }
  if (host.exitCode === null && !host.killed) host.kill();
  if (passed && path.dirname(path.resolve(work)) === path.resolve(target)) {
    try {
      rmSync(work, { recursive: true, force: true, maxRetries: 8, retryDelay: 250 });
    } catch (err) {
      if (err.code !== "EPERM" && err.code !== "EBUSY") throw err;
      console.warn("Windows still holds the temporary test directory:", work);
    }
  }
}
