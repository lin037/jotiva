// Linked only into a staged QA application. Events stay in this process.
#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>

int jot_qa_menu(void *pointer, const char *request) {
    NSWindow *window = (__bridge NSWindow *)pointer;
    if (!NSThread.isMainThread || !window) return -1;
    NSString *action = [NSString stringWithUTF8String:request];
    static EventHotKeyRef reserved;
    if ([action isEqualToString:@"reserve"]) {
        EventHotKeyID identifier = {'JTQA', 1};
        return RegisterEventHotKey(kVK_F19, controlKey | shiftKey, identifier,
            GetApplicationEventTarget(), kEventHotKeyExclusive, &reserved) == noErr ? 1 : -2;
    }
    if ([action isEqualToString:@"release"]) {
        if (!reserved || UnregisterEventHotKey(reserved) != noErr) return -2;
        reserved = NULL;
        return 1;
    }
    if (!window.isKeyWindow) return -3;
    BOOL post = [action hasPrefix:@"post-"];
    NSString *key = [action componentsSeparatedByString:@"-"].lastObject;
    unsigned short code;
    if ([key isEqualToString:@"w"]) code = kVK_ANSI_W;
    else if ([key isEqualToString:@"j"]) code = kVK_ANSI_J;
    else if ([key isEqualToString:@","]) code = kVK_ANSI_Comma;
    else return -4;
    NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint
        modifierFlags:NSEventModifierFlagCommand timestamp:NSProcessInfo.processInfo.systemUptime
        windowNumber:window.windowNumber context:nil characters:key charactersIgnoringModifiers:key
        isARepeat:NO keyCode:code];
    if (post) {
        [NSApp postEvent:event atStart:NO];
        return 1;
    }
    return [NSApp.mainMenu performKeyEquivalent:event];
}
