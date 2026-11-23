// A separate AppKit process owns two documents and a panel that can take keyboard
// focus without becoming the main document. Run the actual Jotiva observer in the parent.
#import <ApplicationServices/ApplicationServices.h>

// Model the activation interval captured on WeCom, while subsequent reads and
// every other AX query go to the real AppKit process.
static unsigned empty_focus_reads;
static unsigned focus_reads;
static AXError read_attributes(AXUIElementRef item, CFArrayRef attributes, AXCopyMultipleAttributeOptions options, CFArrayRef *result) {
    if (CFArrayGetCount(attributes) == 2 && CFEqual(CFArrayGetValueAtIndex(attributes, 0), kAXFocusedWindowAttribute)) {
        ++focus_reads;
        if (empty_focus_reads) {
            --empty_focus_reads;
            *result = NULL;
            return kAXErrorCannotComplete;
        }
    }
    return AXUIElementCopyMultipleAttributeValues(item, attributes, options, result);
}
#define AXUIElementCopyMultipleAttributeValues read_attributes
#import "../../src-tauri/src/host/macos.m"
#undef AXUIElementCopyMultipleAttributeValues

@interface JotKeyboardPanel : NSPanel
@end

@implementation JotKeyboardPanel
- (BOOL)canBecomeMainWindow { return NO; }
@end

static void click_document(NSWindow *window) {
    NSRect frame = window.frame;
    NSPoint visible = NSZeroPoint;
    CGFloat fractions[] = { .5, .2, .8 };
    for (size_t y = 0; y < 3 && NSEqualPoints(visible, NSZeroPoint); ++y) {
        for (size_t x = 0; x < 3; ++x) {
            NSPoint at = NSMakePoint(NSMinX(frame) + NSWidth(frame) * fractions[x],
                                     NSMinY(frame) + NSHeight(frame) * fractions[y]);
            if ([NSWindow windowNumberAtPoint:at belowWindowWithWindowNumber:0] == window.windowNumber) {
                visible = at;
                break;
            }
        }
    }
    if (NSEqualPoints(visible, NSZeroPoint)) {
        fprintf(stderr, "No exposed point on observer fixture window %ld\n", (long)window.windowNumber);
        return;
    }
    CGPoint point = CGPointMake(visible.x, NSMaxY(NSScreen.screens.firstObject.frame) - visible.y);
    CGEventRef event = CGEventCreateMouseEvent(NULL, kCGEventLeftMouseDown, point, kCGMouseButtonLeft);
    CGEventPost(kCGHIDEventTap, event);
    CGEventSetType(event, kCGEventLeftMouseUp);
    CGEventPost(kCGHIDEventTap, event);
    CFRelease(event);
}

static void run_host(void) {
    NSApplication *app = NSApplication.sharedApplication;
    [app setActivationPolicy:NSApplicationActivationPolicyRegular];
    NSUInteger style = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable;
    NSRect screen = NSScreen.mainScreen.visibleFrame;
    NSRect frame = NSMakeRect(NSMinX(screen) + 24, NSMidY(screen), 220, 140);
    NSWindow *a = [[NSWindow alloc] initWithContentRect:frame styleMask:style backing:NSBackingStoreBuffered defer:NO];
    NSWindow *b = [[NSWindow alloc] initWithContentRect:NSOffsetRect(frame, 265, -20)
                                             styleMask:style backing:NSBackingStoreBuffered defer:NO];
    NSPanel *panel = [[JotKeyboardPanel alloc] initWithContentRect:NSOffsetRect(frame, 80, -210)
                                                        styleMask:style backing:NSBackingStoreBuffered defer:NO];
    a.title = @"Focus-A";
    b.title = @"Focus-B";
    panel.title = @"Utility";
    [app finishLaunching];
    [app activateIgnoringOtherApps:YES];
    [b orderFront:nil];
    [a makeKeyAndOrderFront:nil];
    puts("ready");
    fflush(stdout);

    dispatch_async(dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^{
        char line[32];
        while (fgets(line, sizeof(line), stdin)) {
            int command = line[0];
            dispatch_async(dispatch_get_main_queue(), ^{
                switch (command) {
                    case 'P': [panel makeKeyAndOrderFront:nil]; break;
                    case 'A': [a makeMainWindow]; break;
                    case 'B': [b makeMainWindow]; break;
                    case 'C': click_document(a); break;
                    case 'I': printf("%ld\n", (long)b.windowNumber); fflush(stdout); break;
                }
            });
        }
        dispatch_async(dispatch_get_main_queue(), ^{ [app terminate:nil]; });
    });
    [app run];
}

static uint64_t current;
static unsigned callbacks;

static void updated(uint64_t closed, bool space_changed) {
    (void)space_changed;
    (void)closed;
    current = jot_host_focused(false).id;
    ++callbacks;
}

static bool until(bool (^condition)(void)) {
    NSDate *end = [NSDate dateWithTimeIntervalSinceNow:4];
    do {
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, .025, false);
        if (condition()) return true;
    } while (end.timeIntervalSinceNow > 0);
    return false;
}

static bool key_and_main(pid_t pid, NSString *key, NSString *main) {
    AXUIElementRef app = AXUIElementCreateApplication(pid);
    NSArray *attributes = @[(__bridge NSString *)kAXFocusedWindowAttribute, (__bridge NSString *)kAXMainWindowAttribute];
    CFArrayRef result = NULL;
    AXUIElementCopyMultipleAttributeValues(app, (__bridge CFArrayRef)attributes, 0, &result);
    CFRelease(app);
    NSArray *windows = CFBridgingRelease(result);
    if (windows.count != 2) return false;
    NSArray *titles = @[key, main];
    for (NSUInteger i = 0; i < windows.count; ++i) {
        AXUIElementRef window = (__bridge AXUIElementRef)windows[i];
        if (CFGetTypeID(window) != AXUIElementGetTypeID()) return false;
        CFTypeRef title = NULL;
        AXUIElementCopyAttributeValue(window, kAXTitleAttribute, &title);
        if (![CFBridgingRelease(title) isEqual:titles[i]]) return false;
    }
    return true;
}

static int exercise(pid_t pid, NSFileHandle *commands) {
    void (^send)(NSString *) = ^(NSString *command) {
        [commands writeData:[[command stringByAppendingString:@"\n"] dataUsingEncoding:NSUTF8StringEncoding]];
    };
    if (!until(^bool{
        return NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == pid &&
               key_and_main(pid, @"Focus-A", @"Focus-A");
    })) return 3;
    uint64_t idA = jot_host_focused(true).id;
    if (!idA) return 4;
    send(@"P");
    if (!until(^bool{ return key_and_main(pid, @"Utility", @"Focus-A"); })) return 5;
    if (jot_host_focused(false).id != idA) {
        puts("Lost attached document when an auxiliary window took keyboard focus");
        return 6;
    }
    unsigned start = callbacks;
    send(@"B");
    if (!until(^bool{ return callbacks > start && !current && key_and_main(pid, @"Utility", @"Focus-B"); })) {
        puts("Missed main-window change to unbound document");
        return 7;
    }
    if (!jot_host_focused(false).has_window) {
        puts("A usable unbound document was mistaken for a pending AX response");
        return 7;
    }
    start = callbacks;
    send(@"A");
    if (!until(^bool{ return callbacks > start && current == idA && key_and_main(pid, @"Utility", @"Focus-A"); })) {
        puts("Missed main-window return while utility kept focus");
        return 8;
    }
    send(@"B");
    if (!until(^bool{ return !current && key_and_main(pid, @"Utility", @"Focus-B"); })) return 9;
    send(@"C");
    if (!until(^bool{ return current == idA && key_and_main(pid, @"Focus-A", @"Focus-A"); })) {
        fprintf(stderr, "Click fixture PID %d front %d: expected %llu observed %llu direct %llu keyAndMainA %d\n",
                pid, NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier, idA, current,
                jot_host_focused(false).id, key_and_main(pid, @"Focus-A", @"Focus-A"));
        return 10;
    }
    // AppKit already published its focus; no more AX events arrive. Drop the
    // activation read and require the observer itself to discover the document.
    CFRunLoopRunInMode(kCFRunLoopDefaultMode, .2, false);
    begin_activation(pid);
    empty_focus_reads = 1;
    updated(0, false);
    if (current || !until(^bool{ return current == idA; })) return 11;

    // Leaving while AX is unavailable must invalidate the scheduled retry.
    begin_activation(pid);
    empty_focus_reads = 8;
    updated(0, false);
    unsigned after_leave = focus_reads;
    begin_activation(0);
    CFRunLoopRunInMode(kCFRunLoopDefaultMode, .2, false);
    if (focus_reads != after_leave) return 12;

    // A non-responding front app incurs the one activation read and at most
    // three further reads. Verify that the chain actually stops.
    begin_activation(pid);
    unsigned before_limit = focus_reads;
    updated(0, false);
    if (!until(^bool{ return focus_reads - before_limit >= 4; })) return 13;
    CFRunLoopRunInMode(kCFRunLoopDefaultMode, .3, false);
    if (focus_reads - before_limit != 4 || current) return 14;
    empty_focus_reads = 0;
    puts("Recovered a registered document after a silent activation; cancelled on departure and stopped at three retries.");
    puts("Registered document followed main-window changes with stable utility focus, hid for its unbound sibling and returned on a real mouse click.");
    return 0;
}

int main(int argc, char **argv) {
    @autoreleasepool {
        if (argc == 2) {
            run_host();
            return 0;
        }
        [NSApplication sharedApplication];
        if (!AXIsProcessTrusted() || !CGPreflightPostEventAccess()) return 2;
        jot_host_watch(updated);
        NSPipe *commands = NSPipe.pipe;
        NSPipe *events = NSPipe.pipe;
        NSTask *task = NSTask.new;
        task.executableURL = [NSURL fileURLWithPath:@(argv[0])];
        task.arguments = @[@"host"];
        task.standardInput = commands;
        task.standardOutput = events;
        if (![task launchAndReturnError:nil]) return 3;
        @try {
            if (!events.fileHandleForReading.availableData.length) return 3;
            return exercise(task.processIdentifier, commands.fileHandleForWriting);
        } @finally {
            [commands.fileHandleForWriting closeFile];
            [task terminate];
            [task waitUntilExit];
        }
    }
}
