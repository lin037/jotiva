// JXA fixture. Use the same desktop thumbnails a person would select in Mission Control.
ObjC.import("AppKit");
ObjC.import("CoreGraphics");
// The Dock exposes AXPress on thumbnails but leaves AXSelectedChildren empty.
// Read the active Space ID in this test process; all switches use the Dock UI.
ObjC.bindFunction("CGSMainConnectionID", ["int", []]);
ObjC.bindFunction("CGSCopyManagedDisplaySpaces", ["id", ["int"]]);

function run([screenIndex, left, top, requested]) {
  const screen = $.NSScreen.screens.js[Number(screenIndex)];
  const displayID = ObjC.unwrap(screen.deviceDescription.objectForKey("NSScreenNumber"));
  const uuid = $.CFUUIDCreateString(null, $.CGDisplayCreateUUIDFromDisplayID(displayID));
  const identifier = ObjC.unwrap(ObjC.castRefToObject(uuid));
  const liveMonitor = () => ObjC.deepUnwrap($.CGSCopyManagedDisplaySpaces($.CGSMainConnectionID()))
    .find(item => item["Display Identifier"] === identifier);
  const monitor = liveMonitor();
  const current = monitor.Spaces.findIndex(space => space.ManagedSpaceID === monitor["Current Space"].ManagedSpaceID);
  const dock = Application("System Events").processes.byName("Dock");
  const overview = () => dock.uiElements().find(item => /^(Mission Control|调度中心)$/.test(item.name() || ""));
  $.NSWorkspace.sharedWorkspace.launchApplication("Mission Control");
  try {
    const limit = Date.now() + 3500;
    let root;
    while (!(root = overview()) && Date.now() < limit) delay(0.08);
    const display = root.uiElements().find(item => {
      const [x, y] = item.position();
      return Math.abs(x - Number(left)) < 3 && Math.abs(y - Number(top)) < 60;
    });
    const bar = display.uiElements().find(item => /^(Spaces Bar|空间栏)$/i.test(item.name() || ""));
    const list = bar.uiElements().find(item => item.role() === "AXList");
    const thumbnails = list.uiElements();
    const selected = thumbnails[current].name();
    const index = thumbnails.findIndex(item => {
      const name = item.name() || "";
      return requested === "other" ? /^(桌面|Desktop)\s*\d+$/.test(name) && name !== selected : name === requested;
    });
    if (index < 0) throw new Error("No matching ordinary desktop on this display");
    thumbnails[index].actions.byName("AXPress").perform();
    const destination = monitor.Spaces[index].ManagedSpaceID;
    const deadline = Date.now() + 3500;
    while (liveMonitor()["Current Space"].ManagedSpaceID !== destination) {
      if (Date.now() >= deadline) throw new Error("The Dock did not select the requested desktop");
      delay(0.05);
    }
    return selected;
  } catch (error) {
    if (overview()) $.NSWorkspace.sharedWorkspace.launchApplication("Mission Control");
    throw error;
  }
}
