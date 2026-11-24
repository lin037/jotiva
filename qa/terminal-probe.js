// Exercise the emulator through paste and key events in the real desktop WebView.
export async function checkTerminal(os, waitFor, run, report) {
  const before = (await run("plugin:window|get_all_windows")).sort().join(",");
  const toggle = document.querySelector('[aria-controls="workspace-terminal"]');
  toggle.click();
  const panel = await waitFor(() => document.querySelector("#workspace-terminal:not([hidden])"), "embedded terminal");
  let input = await waitFor(() => panel.querySelector("textarea:focus"), "shell ready");
  const key = (key, code, control = false) => {
    for (const type of ["keydown", "keyup"]) input.dispatchEvent(new KeyboardEvent(type, {
      key, code, keyCode: key === "Enter" ? 13 : 67, which: key === "Enter" ? 13 : 67,
      ctrlKey: control, bubbles:true, cancelable:true
    }));
  };
  const paste = line => {
    const data = new DataTransfer();
    data.setData("text/plain", line);
    input.dispatchEvent(new ClipboardEvent("paste", {clipboardData:data, bubbles:true, cancelable:true}));
  };
  const send = line => { paste(line); key("Enter", "Enter"); };
  const contains = marker => panel.querySelector(".terminal-screen:not([hidden]) .xterm-rows")?.textContent.includes(marker);
  const saved = os === "windows"
    ? "[IO.File]::WriteAllLines((Join-Path (Get-Location) terminal-context.txt), @((Get-Location).Path, $env:JOTIVA_INBOX_DIR)); $keep = 641; Write-Output ('cwd-' + $keep)"
    : 'test -t 0 && test -t 1 && printf "%s\\n" "$PWD" "$JOTIVA_INBOX_DIR" > terminal-context.txt; keep=641; echo "cwd-$keep"';
  send(saved);
  await waitFor(() => contains("cwd-641"), "interactive terminal and cwd");
  toggle.click();
  await waitFor(() => panel.hidden, "terminal collapsed");
  toggle.click();
  await waitFor(() => !panel.hidden && panel.contains(document.activeElement), "terminal regained focus");
  const handle = panel.querySelector("[role=separator]");
  const height = panel.offsetHeight;
  handle.dispatchEvent(new KeyboardEvent("keydown", {key:"ArrowUp", bubbles:true, cancelable:true}));
  await waitFor(() => panel.offsetHeight > height, "resized terminal");
  send(os === "windows" ? "Write-Output ('sleep-' + $keep); Start-Sleep -Seconds 45" : 'echo "sleep-$keep"; sleep 45');
  await waitFor(() => contains("sleep-641"), "same shell after hiding");
  const initial = await run("initial_note");
  const [root] = await run("workspace_roots");
  const sep = os === "windows" ? "\\" : "/";
  const nestedFile = [root, "posts", "a ' $ file.md"].join(sep);
  const nested = await run("open_library_file", {path:nestedFile});
  await run("show_main", {id:nested});
  const tab = await waitFor(() => document.querySelector('[role=tab][aria-selected=true][data-note="' + nested + '"]'), "nested document");
  const shortcut = target => target.dispatchEvent(new KeyboardEvent("keydown", {key:"~", code:"Backquote", ctrlKey:true, shiftKey:true, bubbles:true, cancelable:true}));
  const chooser = panel.querySelector("select");
  if (os === "windows") {
    tab.focus();
    await report("input " + JSON.stringify({name:"new terminal for current tab", title:await run("plugin:window|title"), keys:["ControlKey","ShiftKey","Oemtilde"]}));
  } else shortcut(tab);
  await waitFor(() => chooser.options.length === 2 && chooser.selectedOptions[0]?.text === "posts", "new terminal in nested document directory");
  chooser.selectedIndex = 0;
  chooser.dispatchEvent(new Event("change"));
  await waitFor(() => contains("sleep-641"), "switch back to the running foreground command");
  chooser.selectedIndex = 1;
  chooser.dispatchEvent(new Event("change"));
  input = await waitFor(() => panel.querySelector("textarea:focus"), "second shell");
  send(os === "windows" ? "Write-Output ('dir-' + (Split-Path -Leaf (Get-Location)))" : 'echo "dir-${PWD##*/}"');
  await waitFor(() => contains("dir-posts"), "real nested cwd");
  paste(os === "windows" ? "Get-Content -LiteralPath " : "cat ");
  const transfer = new DataTransfer();
  tab.dispatchEvent(new DragEvent("dragstart", {dataTransfer:transfer, bubbles:true}));
  panel.querySelector(".terminal-screen:not([hidden])").dispatchEvent(new DragEvent("drop", {dataTransfer:transfer, bubbles:true, cancelable:true}));
  tab.dispatchEvent(new DragEvent("dragend", {dataTransfer:transfer, bubbles:true}));
  await waitFor(() => contains("file.md"), "quoted tab path inserted");
  if (contains("jotiva-drop-proof")) throw Error("Drop submitted command");
  key("Enter", "Enter");
  await waitFor(() => contains("jotiva-drop-proof"), "shell resolved dropped path");
  const close = panel.querySelector(".terminal-heading button:last-child");
  // Closing the document that started the foreground task must preserve the session.
  document.querySelector('[role=tab][data-note="' + initial + '"]').parentElement.querySelector(".document-tab-close").click();
  await waitFor(() => !document.querySelector('[role=tab][data-note="' + initial + '"]'), "originating tab closed");
  await run("quit");
  const dialog = await waitFor(() => document.querySelector("dialog[open] .sheet-actions"), "quit confirmation");
  dialog.querySelector("button").click();
  await waitFor(() => !document.querySelector("dialog[open]") && !document.querySelector("#app").inert, "cancel quit");
  if (await run("live_terminals") !== 2) throw Error("Cancel or tab close ended a session");
  // Verify the other native origins as well as the UI shortcut.
  const inbox = await run("inbox_directory");
  for (const [location, expected] of [[{kind:"workspace",path:root}, root], [{kind:"workspace",path:nestedFile}, [root,"posts"].join(sep)],
      [{kind:"archive",folder:null}, inbox], [{kind:"document",id:null}, inbox]]) {
    const result = await run("open_terminal",{location,cols:80,rows:24}).catch(error => { throw Error(JSON.stringify(location) + ": " + error); });
    if (result.directory !== expected) throw Error("Incorrect terminal origin " + result.directory);
    await run("close_terminal",{id:result.id});
  }
  close.click();
  await waitFor(() => chooser.options.length === 1 && panel.querySelector("textarea:focus"), "first session restored");
  input = panel.querySelector("textarea:focus");
  if (os === "windows") {
    await report("input " + JSON.stringify({name:"interrupt terminal", title:await run("plugin:window|title"), keys:["ControlKey","C"]}));
    // PowerShell discards type-ahead while interrupting its pipeline. Wait for its next prompt.
    await waitFor(() => {
      const rows = panel.querySelector(".xterm-rows")?.textContent ?? "";
      return rows.slice(rows.lastIndexOf("sleep-641") + 9).includes("PS ");
    }, "PowerShell prompt after Ctrl+C").catch(e => { throw Error(e.message + " " + panel.querySelector(".xterm-rows")?.innerText); });
  } else key("c", "KeyC", true);
  send(os === "windows" ? "Write-Output ('interrupt-' + $keep)" : 'echo "interrupt-$keep"');
  await waitFor(() => contains("interrupt-641"), "Ctrl+C reached the shell");
  send("exit");
  await waitFor(() => !panel.querySelector("[role=status]").hidden, "shell exit");
  if (before !== (await run("plugin:window|get_all_windows")).sort().join(",")) throw Error("Terminal opened a window");
  panel.querySelector(".terminal-heading button").click();
  input = await waitFor(() => { const next = panel.querySelector("textarea:focus"); return next !== input && next; }, "fresh terminal");
  send(os === "windows"
    ? "[IO.File]::WriteAllText((Join-Path (Get-Location) terminal-pid.txt), [string]$PID); if ($null -eq $keep) {Write-Output ('fresh-' + 321)}; Start-Sleep -Seconds 45"
    : 'echo $$ > terminal-pid.txt; test -z "$keep" && printf "fresh-%d\\n" 321; sleep 45');
  await waitFor(() => contains("fresh-321"), "independent new session");
  panel.querySelector(".terminal-heading button:last-child").click();
  await waitFor(() => chooser.options.length === 1, "ended transcript preserved");
  panel.querySelector(".terminal-heading button:last-child").click();
  await waitFor(() => panel.hidden, "terminal closed");
  await run("show_main", {id:initial});
  await waitFor(() => document.querySelector('[role=tab][data-note="' + initial + '"][aria-selected=true]'), "original document reopened");
  document.querySelector('[role=tab][data-note="' + nested + '"]').parentElement.querySelector(".document-tab-close").click();
}
