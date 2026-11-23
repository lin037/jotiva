#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>

static const OSType signature = 'JOTD';
static NSMutableDictionary<NSNumber *, NSValue *> *registered;
static void (*pressed)(uint32_t);

static OSStatus handle_press(EventHandlerCallRef next, EventRef event, void *context) {
    (void)next;
    (void)context;
    EventHotKeyID key;
    OSStatus result = GetEventParameter(event, kEventParamDirectObject, typeEventHotKeyID,
        NULL, sizeof(key), NULL, &key);
    if (result != noErr || key.signature != signature) return eventNotHandledErr;
    pressed(key.id);
    return noErr;
}

OSStatus jot_hotkeys_start(void (*callback)(uint32_t)) {
    pressed = callback;
    registered = [NSMutableDictionary dictionary];
    EventTypeSpec event = {kEventClassKeyboard, kEventHotKeyPressed};
    return InstallApplicationEventHandler(handle_press, 1, &event, NULL, NULL);
}

// WebKit KeyboardEvent.code and Carbon both describe the physical key.
static const struct { const char *name; UInt16 code; } keys[] = {
    {"KeyA", kVK_ANSI_A}, {"KeyB", kVK_ANSI_B}, {"KeyC", kVK_ANSI_C},
    {"KeyD", kVK_ANSI_D}, {"KeyE", kVK_ANSI_E}, {"KeyF", kVK_ANSI_F},
    {"KeyG", kVK_ANSI_G}, {"KeyH", kVK_ANSI_H}, {"KeyI", kVK_ANSI_I},
    {"KeyJ", kVK_ANSI_J}, {"KeyK", kVK_ANSI_K}, {"KeyL", kVK_ANSI_L},
    {"KeyM", kVK_ANSI_M}, {"KeyN", kVK_ANSI_N}, {"KeyO", kVK_ANSI_O},
    {"KeyP", kVK_ANSI_P}, {"KeyQ", kVK_ANSI_Q}, {"KeyR", kVK_ANSI_R},
    {"KeyS", kVK_ANSI_S}, {"KeyT", kVK_ANSI_T}, {"KeyU", kVK_ANSI_U},
    {"KeyV", kVK_ANSI_V}, {"KeyW", kVK_ANSI_W}, {"KeyX", kVK_ANSI_X},
    {"KeyY", kVK_ANSI_Y}, {"KeyZ", kVK_ANSI_Z}, {"Digit0", kVK_ANSI_0},
    {"Digit1", kVK_ANSI_1}, {"Digit2", kVK_ANSI_2}, {"Digit3", kVK_ANSI_3},
    {"Digit4", kVK_ANSI_4}, {"Digit5", kVK_ANSI_5}, {"Digit6", kVK_ANSI_6},
    {"Digit7", kVK_ANSI_7}, {"Digit8", kVK_ANSI_8}, {"Digit9", kVK_ANSI_9},
    {"F1", kVK_F1}, {"F2", kVK_F2}, {"F3", kVK_F3},
    {"F4", kVK_F4}, {"F5", kVK_F5}, {"F6", kVK_F6},
    {"F7", kVK_F7}, {"F8", kVK_F8}, {"F9", kVK_F9},
    {"F10", kVK_F10}, {"F11", kVK_F11}, {"F12", kVK_F12},
    {"F13", kVK_F13}, {"F14", kVK_F14}, {"F15", kVK_F15},
    {"F16", kVK_F16}, {"F17", kVK_F17}, {"F18", kVK_F18},
    {"F19", kVK_F19}, {"F20", kVK_F20}, {"Space", kVK_Space},
    {"Enter", kVK_Return}, {"Tab", kVK_Tab}, {"Backspace", kVK_Delete},
    {"ArrowLeft", kVK_LeftArrow}, {"ArrowUp", kVK_UpArrow}, {"ArrowRight", kVK_RightArrow},
    {"ArrowDown", kVK_DownArrow}, {"Comma", kVK_ANSI_Comma}, {"Period", kVK_ANSI_Period},
    {"Slash", kVK_ANSI_Slash}, {"Backslash", kVK_ANSI_Backslash}, {"Backquote", kVK_ANSI_Grave},
    {"Semicolon", kVK_ANSI_Semicolon}, {"Quote", kVK_ANSI_Quote}, {"Minus", kVK_ANSI_Minus},
    {"Equal", kVK_ANSI_Equal}, {"BracketLeft", kVK_ANSI_LeftBracket}, {"BracketRight", kVK_ANSI_RightBracket},
};

static int virtual_key(const char *key) {
    for (size_t i = 0; i < sizeof(keys) / sizeof(keys[0]); ++i) {
        if (!strcmp(keys[i].name, key)) return keys[i].code;
    }
    return -1;
}

static NSString *recorded_key(NSEvent *event) {
    NSEventModifierFlags flags = event.modifierFlags;
    for (size_t i = 0; i < sizeof(keys) / sizeof(keys[0]); ++i) {
        if (keys[i].code != event.keyCode) continue;
        const char *name = keys[i].name;
        if (!(flags & (NSEventModifierFlagCommand | NSEventModifierFlagOption | NSEventModifierFlagControl))
            && name[0] != 'F') return nil;
        NSMutableString *chord = [NSMutableString string];
        if (flags & NSEventModifierFlagControl) [chord appendString:@"Control+"];
        if (flags & NSEventModifierFlagOption) [chord appendString:@"Alt+"];
        if (flags & NSEventModifierFlagShift) [chord appendString:@"Shift+"];
        if (flags & NSEventModifierFlagCommand) [chord appendString:@"Super+"];
        [chord appendString:[NSString stringWithUTF8String:name]];
        return chord;
    }
    return nil;
}

// AppKit handles menu key equivalents before WKWebView receives keydown.
// Use a local monitor only while the user records in the settings window.
void jot_record_shortcut(void *window, void (*recorded)(const char *)) {
    static id monitor;
    void (^change)(void) = ^{
        if (monitor) { [NSEvent removeMonitor:monitor]; monitor = nil; }
        if (!window) return;
        __weak NSWindow *settings = (__bridge NSWindow *)window;
        monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:^NSEvent *(NSEvent *event) {
            if (!settings || !settings.isKeyWindow || event.window != settings) return event;
            NSString *chord = recorded_key(event);
            if (!chord) return event;
            if (!event.isARepeat) recorded(chord.UTF8String);
            return nil;
        }];
    };
    if (NSThread.isMainThread) change(); else dispatch_sync(dispatch_get_main_queue(), change);
}

OSStatus jot_hotkey_register(uint32_t id, const char *key, uint32_t modifiers) {
    int code = virtual_key(key);
    if (code < 0) return paramErr;
    UInt32 flags = ((modifiers & 1) ? controlKey : 0) |
        ((modifiers & 2) ? shiftKey : 0) |
        ((modifiers & 4) ? optionKey : 0) |
        ((modifiers & 8) ? cmdKey : 0);
    EventHotKeyRef registration;
    OSStatus result = RegisterEventHotKey((UInt32)code, flags, (EventHotKeyID){signature, id},
        GetApplicationEventTarget(), kEventHotKeyExclusive, &registration);
    if (result == noErr) registered[@(id)] = [NSValue valueWithPointer:registration];
    return result;
}

OSStatus jot_hotkey_unregister(uint32_t id) {
    NSNumber *key = @(id);
    EventHotKeyRef registration = registered[key].pointerValue;
    if (!registration) return noErr;
    OSStatus result = UnregisterEventHotKey(registration);
    if (result == noErr) [registered removeObjectForKey:key];
    return result;
}
