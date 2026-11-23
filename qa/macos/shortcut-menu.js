// This probe runs in the real WKWebViews of a temporary installation.
const api = window.__TAURI_INTERNALS__;
const run = (cmd, args) => api.invoke(cmd, args);
const key = action => run("qa_frame", { action });
async function record(name, data = {}) {
  const id = await run("new_note");
  await run("save_note", { id, version: 0, markdown: "#probe:" + name + "\n" + JSON.stringify(data) });
}
async function wait(check, description) {
  for (let i = 0; i < 140; i++) {
    if (await check()) return;
    await new Promise(resolve => setTimeout(resolve, 40));
  }
  throw Error(description);
}

async function verify() {
  await wait(() => document.querySelector(".settings-nav"), "settings did not mount");
  const { preferences } = await run("read_settings");
  if (preferences.local.settings === "Super+KeyJ") {
    if (await key("menu-,") || !await key("menu-j")) throw Error("Restored menu does not match saved preferences");
    await run("save_settings", {preferences: {...preferences, local:{settings:null}}});
    if (await key("menu-j") || await key("menu-,")) throw Error("A cleared menu shortcut still runs");
    await record("restored");
    return;
  }
  if (!await key("menu-,") || await key("menu-j")) throw Error("Unexpected default settings menu shortcuts");
  const recorder = document.querySelector("#key-local-settings");
  const tab = [...document.querySelectorAll(".settings-nav button")].find(e => e.textContent === "快捷键");
  tab.click();
  const received = [];
  await run("plugin:event|listen", {event:"desktop:shortcut-recorded", target:{kind:"Any"},
    handler:api.transformCallback(event => received.push(event.payload))});
  for (const letter of ["w","j"]) {
    recorder.click();
    await wait(() => recorder.getAttribute("aria-pressed") === "true", "recorder did not start");
    await key("post-" + letter);
    await wait(() => received.at(-1) === "Super+Key" + letter.toUpperCase() &&
      recorder.getAttribute("aria-pressed") === "false", "AppKit did not record Cmd+" + letter);
  }
  recorder.closest("form").requestSubmit();
  await wait(async () => (await run("read_settings")).preferences.local.settings === "Super+KeyJ", "Save did not persist");
  if (await key("menu-,") || !await key("menu-j")) throw Error("Live AppKit key equivalent did not update");
  // Ask the native registrar to reserve a chord for this process before trying
  // to bind the same key from Jotiva. The rejected save must restore the menu.
  await key("reserve");
  let error;
  try {
    await run("save_settings", {preferences:{...preferences, search:"Control+Shift+F19", local:{settings:null}}});
  } catch (failure) { error = String(failure); }
  finally { await key("release"); }
  if (!error?.includes("独占登记") || !await key("menu-j")) throw Error("Carbon conflict or menu rollback failed: " + error);
  await record("remapped");
}
try {
  if (api.metadata.currentWindow.label === "main") await run("show_settings", {section:"shortcuts"});
  if (api.metadata.currentWindow.label === "settings") await verify();
} catch (error) { await record("error", String(error)); }
