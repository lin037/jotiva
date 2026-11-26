#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#import <Carbon/Carbon.h>
#include <string.h>
#include <unistd.h>

static AXUIElementRef copy_quick_window(pid_t jot_pid) {
    AXUIElementRef application = AXUIElementCreateApplication(jot_pid);
    CFTypeRef raw = NULL;
    AXUIElementCopyAttributeValue(application, kAXWindowsAttribute, &raw);
    CFRelease(application);
    NSArray *windows = CFBridgingRelease(raw);
    for (id window in windows) {
        AXUIElementRef candidate = (__bridge AXUIElementRef)window;
        CFTypeRef title = NULL;
        AXUIElementCopyAttributeValue(candidate, kAXTitleAttribute, &title);
        if ([CFBridgingRelease(title) isEqual:@"Jotiva 便签"]) {
            CFRetain(candidate);
            return candidate;
        }
    }
    return NULL;
}

static id read_attribute(id element, CFStringRef attribute) {
    CFTypeRef raw = NULL;
    if (element) AXUIElementCopyAttributeValue((__bridge AXUIElementRef)element, attribute, &raw);
    return CFBridgingRelease(raw);
}

static NSString *close_command(pid_t pid) {
    id app = CFBridgingRelease(AXUIElementCreateApplication(pid));
    id front = read_attribute(read_attribute(app, kAXFocusedWindowAttribute), kAXTitleAttribute);
    NSArray *bar = read_attribute(read_attribute(app, kAXMenuBarAttribute), kAXChildrenAttribute);
    for (id group in bar) {
        if (![read_attribute(group, kAXTitleAttribute) isEqual:@"文件"]) continue;
        id menu = [read_attribute(group, kAXChildrenAttribute) firstObject];
        for (id row in read_attribute(menu, kAXChildrenAttribute)) {
            NSString *key = read_attribute(row, kAXMenuItemCmdCharAttribute);
            if ([key.lowercaseString isEqual:@"w"]) {
                return [NSString stringWithFormat:@"keyWindow=%@ command=%@ enabled=%@", front,
                        read_attribute(row, kAXTitleAttribute), read_attribute(row, kAXEnabledAttribute)];
            }
        }
    }
    return [NSString stringWithFormat:@"keyWindow=%@ no Cmd+W", front];
}

static BOOL quick_frame(pid_t pid, CGRect *frame) {
    AXUIElementRef window = copy_quick_window(pid);
    if (!window) return NO;
    CFTypeRef pos = NULL, size = NULL;
    AXUIElementCopyAttributeValue(window, kAXPositionAttribute, &pos);
    AXUIElementCopyAttributeValue(window, kAXSizeAttribute, &size);
    BOOL ok = pos && size && AXValueGetValue(pos,kAXValueCGPointType,&frame->origin) && AXValueGetValue(size,kAXValueCGSizeType,&frame->size);
    if (pos) CFRelease(pos); if (size) CFRelease(size); CFRelease(window);
    return ok;
}

// An AX window may still expose geometry after it was ordered out. Checking
// the actual hit point also catches a note stranded behind its host.
static BOOL quick_on_top_at_center(pid_t pid, CGRect frame) {
    NSPoint point = NSMakePoint(CGRectGetMidX(frame), NSMaxY(NSScreen.screens.firstObject.frame) - CGRectGetMidY(frame));
    NSInteger hit = [NSWindow windowNumberAtPoint:point belowWindowWithWindowNumber:0];
    NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionIncludingWindow, (CGWindowID)hit));
    return [windows.firstObject[(__bridge NSString *)kCGWindowOwnerPID] intValue] == pid;
}

static void resize_quick(CGRect frame) {
    // Drag the bottom-right resize region inward so this also works at a screen edge.
    CGPoint corner = CGPointMake(CGRectGetMaxX(frame)-3, CGRectGetMaxY(frame)-5);
    for (int i = -1; i <= 19; ++i) {
        double step = MAX(0, MIN(i, 18));
        CGPoint point = CGPointMake(corner.x - step*3, corner.y - step*2);
        CGEventType type = i < 0 ? kCGEventMouseMoved : i == 0 ? kCGEventLeftMouseDown :
            i == 19 ? kCGEventLeftMouseUp : kCGEventLeftMouseDragged;
        CGEventRef event = CGEventCreateMouseEvent(NULL,type,point,kCGMouseButtonLeft);
        CGEventPost(kCGHIDEventTap,event); CFRelease(event); usleep(18000);
    }
}

static AXError click_quick_close(pid_t jot_pid) {
    AXUIElementRef window = copy_quick_window(jot_pid);
    if (!window) return kAXErrorNoValue;
    CFTypeRef button = NULL;
    AXError found = AXUIElementCopyAttributeValue(window, kAXCloseButtonAttribute, &button);
    CFRelease(window);
    if (found != kAXErrorSuccess) return found;
    CFTypeRef point = NULL;
    CFTypeRef size = NULL;
    AXUIElementCopyAttributeValue((AXUIElementRef)button, kAXPositionAttribute, &point);
    AXUIElementCopyAttributeValue((AXUIElementRef)button, kAXSizeAttribute, &size);
    CFRelease(button);
    CGPoint origin;
    CGSize dimensions;
    bool located = point && size && AXValueGetValue((AXValueRef)point, kAXValueCGPointType, &origin)
        && AXValueGetValue((AXValueRef)size, kAXValueCGSizeType, &dimensions);
    if (point) CFRelease(point);
    if (size) CFRelease(size);
    if (!located || !CGPreflightPostEventAccess()) return kAXErrorNoValue;
    CGPoint center = CGPointMake(origin.x + dimensions.width / 2, origin.y + dimensions.height / 2);
    NSPoint hit_point = NSMakePoint(center.x, NSMaxY(NSScreen.screens.firstObject.frame) - center.y);
    NSInteger hit = [NSWindow windowNumberAtPoint:hit_point belowWindowWithWindowNumber:0];
    NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionIncludingWindow, (CGWindowID)hit));
    NSNumber *owner = windows.firstObject[(__bridge NSString *)kCGWindowOwnerPID];
    if (owner.intValue != jot_pid) return kAXErrorInvalidUIElement;

    CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStateHIDSystemState);
    CGEventRef motion = CGEventCreateMouseEvent(source, kCGEventMouseMoved, center, kCGMouseButtonLeft);
    CGEventPost(kCGSessionEventTap, motion);
    CFRelease(motion);
    CGEventRef event = CGEventCreateMouseEvent(source, kCGEventLeftMouseDown, center, kCGMouseButtonLeft);
    CGEventSetIntegerValueField(event, kCGMouseEventClickState, 1);
    CGEventPost(kCGHIDEventTap, event);
    usleep(40000);
    CGEventSetType(event, kCGEventLeftMouseUp);
    CGEventPost(kCGHIDEventTap, event);
    CFRelease(event);
    CFRelease(source);
    return kAXErrorSuccess;
}

// Only post input when the fixture owns the pixel under the pointer.
static CGPoint exposed_point(NSWindow *target) {
    CGFloat top = NSMaxY(NSScreen.screens.firstObject.frame);
    for (CGFloat x = NSMaxX(target.frame) - 15; x > NSMinX(target.frame) + 10; x -= 20) {
        for (CGFloat y = NSMinY(target.frame) + 15; y < NSMaxY(target.frame) - 28; y += 20) {
            NSPoint point = NSMakePoint(x, y);
            if ([NSWindow windowNumberAtPoint:point belowWindowWithWindowNumber:0] == target.windowNumber)
                return CGPointMake(x, top - y);
        }
    }
    return CGPointZero;
}

static const char *click_point(CGPoint point) {
    CGEventRef motion = CGEventCreateMouseEvent(NULL, kCGEventMouseMoved, point, kCGMouseButtonLeft);
    CGEventPost(kCGHIDEventTap, motion);
    CFRelease(motion);
    CGEventRef event = CGEventCreateMouseEvent(NULL, kCGEventLeftMouseDown, point, kCGMouseButtonLeft);
    CGEventSetFlags(event, 0);
    CGEventSetIntegerValueField(event, kCGMouseEventClickState, 1);
    CGEventPost(kCGHIDEventTap, event);
    CFRelease(event);
    usleep(40000);
    event = CGEventCreateMouseEvent(NULL, kCGEventLeftMouseUp, point, kCGMouseButtonLeft);
    CGEventSetFlags(event, 0);
    CGEventSetIntegerValueField(event, kCGMouseEventClickState, 1);
    CGEventPost(kCGHIDEventTap, event);
    CFRelease(event);
    return "clicked";
}

static const char *click_host(NSWindow *target) {
    CGPoint point = exposed_point(target);
    return CGPointEqualToPoint(point, CGPointZero) ? "fixture fully occluded" : click_point(point);
}

static const char *drag_file(pid_t jot_pid, CGPoint start, NSWindow *target, BOOL switching) {
    NSScreen *screen = NSScreen.screens.firstObject;
    CGFloat top = NSMaxY(screen.frame);
    NSPoint origin = NSMakePoint(start.x, top - start.y);
    NSInteger hit = [NSWindow windowNumberAtPoint:origin belowWindowWithWindowNumber:0];
    NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionIncludingWindow, (CGWindowID)hit));
    if (!CGPreflightPostEventAccess()) return "no event access";
    if ([windows.firstObject[(__bridge NSString *)kCGWindowOwnerPID] intValue] != jot_pid) {
        CGRect quickFrame = CGRectZero;
        quick_frame(jot_pid, &quickFrame);
        fprintf(stderr, "drag start=(%.1f,%.1f) hit=%ld owner=%s layer=%ld quickFrame=%s\n",
            start.x, start.y, (long)hit, [windows.firstObject[(__bridge NSString *)kCGWindowOwnerName] UTF8String],
            (long)[windows.firstObject[(__bridge NSString *)kCGWindowLayer] integerValue], NSStringFromRect(quickFrame).UTF8String);
        return "start outside Jotiva";
    }
    NSPoint end = target ? exposed_point(target) : NSMakePoint(start.x + (start.x < NSMidX(screen.frame) ? 48 : -48), start.y + 8);
    if (!end.x) {
        fprintf(stderr, "fixture=%ld visible=%d activeSpace=%d frame=%s screen=%s topmost=%ld\n",
            (long)target.windowNumber, target.visible, target.onActiveSpace,
            NSStringFromRect(target.frame).UTF8String, NSStringFromRect(screen.frame).UTF8String,
            (long)[NSWindow windowNumberAtPoint:NSMakePoint(NSMidX(target.frame), NSMidY(target.frame)) belowWindowWithWindowNumber:0]);
        return "fixture fully occluded";
    }
    if (switching) { [target makeKeyWindow]; end = NSMakePoint(start.x + 16, start.y + 20); }
    CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStateHIDSystemState);
    for (int step = -1; step <= 35; step++) {
        double amount = fmin(1., fmax(0., step / 34.));
        CGPoint point = CGPointMake(start.x + amount * (end.x - start.x), start.y + amount * (end.y - start.y));
        CGEventType type = step < 0 ? kCGEventMouseMoved : step == 0 ? kCGEventLeftMouseDown :
            step == 35 ? kCGEventLeftMouseUp : kCGEventLeftMouseDragged;
        CGEventRef event = CGEventCreateMouseEvent(source, type, point, kCGMouseButtonLeft);
        if (step == 0 && (CGEventGetFlags(event) & (kCGEventFlagMaskShift | kCGEventFlagMaskControl | kCGEventFlagMaskCommand | kCGEventFlagMaskAlternate))) {
            fprintf(stderr, "Fixture drag inherited keyboard modifiers: %llu\n", (unsigned long long)CGEventGetFlags(event));
        }
        CGEventSetFlags(event, 0);
        CGEventSetIntegerValueField(event, kCGMouseEventClickState, 1);
        CGEventPost(kCGHIDEventTap, event);
        CFRelease(event);
        if (switching && step == 23) {
            CGKeyCode keys[] = {kVK_Command, kVK_Tab, kVK_Tab, kVK_Command};
            for (int i = 0; i < 4; ++i) {
                CGEventRef key = CGEventCreateKeyboardEvent(source, keys[i], i < 2);
                CGEventSetFlags(key, i == 3 ? 0 : kCGEventFlagMaskCommand);
                CGEventPost(kCGHIDEventTap, key); CFRelease(key); usleep(60000);
            }
            usleep(220000);
        }
        usleep(step == 0 ? 120000 : 14000);
    }
    CFRelease(source);
    return "sent";
}

int main(int argc, char **argv) {
    if (argc == 3 && (strcmp(argv[1], "document-mode") == 0 || strcmp(argv[1], "close-tab") == 0)) {
        pid_t pid = atoi(argv[2]);
        CGKeyCode code = strcmp(argv[1], "close-tab") == 0 ? kVK_ANSI_W : kVK_ANSI_Slash;
        for (int down = 1; down >= 0; --down) {
            CGEventRef event = CGEventCreateKeyboardEvent(NULL, code, down);
            CGEventSetFlags(event, kCGEventFlagMaskCommand);
            CGEventPostToPid(pid, event);
            CFRelease(event);
            usleep(18000);
        }
        return 0;
    }
    if (argc >= 4 && strcmp(argv[1], "open-markdown") == 0) {
        @autoreleasepool {
            NSAppleEventDescriptor *target = [NSAppleEventDescriptor descriptorWithProcessIdentifier:atoi(argv[2])];
            NSAppleEventDescriptor *event = [NSAppleEventDescriptor appleEventWithEventClass:kCoreEventClass
                eventID:kAEOpenDocuments targetDescriptor:target returnID:kAutoGenerateReturnID transactionID:kAnyTransactionID];
            NSAppleEventDescriptor *list = NSAppleEventDescriptor.listDescriptor;
            for (int i = 3; i < argc; i++) {
                NSURL *url = [NSURL fileURLWithPath:[NSString stringWithUTF8String:argv[i]]];
                [list insertDescriptor:[NSAppleEventDescriptor descriptorWithFileURL:url] atIndex:i - 2];
            }
            [event setParamDescriptor:list forKeyword:keyDirectObject];
            NSError *error = nil;
            [event sendEventWithOptions:NSAppleEventSendNoReply timeout:2 error:&error];
            if (error) fprintf(stderr, "%s\n", error.description.UTF8String);
            return error ? 1 : 0;
        }
    }
    if (argc != 3) return 2;
    pid_t jot_pid = (pid_t)strtol(argv[1], NULL, 10);
    CGKeyCode quick_key = strcmp(argv[2], "u") == 0 ? kVK_ANSI_U : kVK_Space;
    @autoreleasepool {
        NSApplication *app = [NSApplication sharedApplication];
        [app setActivationPolicy:NSApplicationActivationPolicyRegular];
        NSWindowStyleMask style = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable;
        NSWindow *a = [[NSWindow alloc] initWithContentRect:NSMakeRect(120, 300, 510, 315)
             styleMask:style backing:NSBackingStoreBuffered defer:NO];
        NSWindow *b = [[NSWindow alloc] initWithContentRect:NSMakeRect(320, 220, 500, 310)
             styleMask:style backing:NSBackingStoreBuffered defer:NO];
        a.releasedWhenClosed = NO;
        b.releasedWhenClosed = NO;
        a.title = @"Fixture A";
        b.title = @"Fixture B";
        __block BOOL fullscreenReady = NO;
        [NSNotificationCenter.defaultCenter addObserverForName:NSWindowDidEnterFullScreenNotification object:a queue:nil usingBlock:^(NSNotification *event) {
            fullscreenReady = YES;
        }];
        [NSNotificationCenter.defaultCenter addObserverForName:NSWindowDidExitFullScreenNotification object:a queue:nil usingBlock:^(NSNotification *event) {
            fullscreenReady = NO;
        }];
        [b orderFront:nil];
        [a makeKeyAndOrderFront:nil];
        [app activateIgnoringOtherApps:YES];
        [app finishLaunching];
        printf("ready %d %d\n", getpid(), CGPreflightPostEventAccess());
        fflush(stdout);
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^{
            char line[128];
            while (fgets(line, sizeof(line), stdin)) {
                NSString *command = [[NSString stringWithUTF8String:line]
                    stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
                dispatch_async(dispatch_get_main_queue(), ^{
                    if ([command isEqualToString:@"A"] || [command isEqualToString:@"B"]) {
                        [app activateIgnoringOtherApps:YES];
                        [[command isEqualToString:@"A"] ? a : b makeKeyAndOrderFront:nil];
                    } else if ([command isEqual:@"frames"]) {
                        NSMutableArray *screens=[NSMutableArray new], *windows=[NSMutableArray new];
                        double top=NSMaxY(NSScreen.screens.firstObject.frame);
                        for (NSScreen *s in NSScreen.screens) {
                            NSRect r=s.visibleFrame;
                            [screens addObject:@[@(r.origin.x),@(top-NSMaxY(r)),@(r.size.width),@(r.size.height)]];
                        }
                        NSArray *info=CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,0));
                        for (NSDictionary *w in info) {
                            if ([w[(id)kCGWindowOwnerPID] intValue]!=jot_pid) continue;
                            NSString *name=w[(id)kCGWindowName];
                            if (!name.length) continue;
                            [windows addObject:@{@"title":name,@"frame":w[(id)kCGWindowBounds]}];
                        }
                        NSDictionary *snapshot=@{@"screens":screens,@"windows":windows,@"hostA":NSStringFromRect(a.frame),@"hostScreen":a.screen.localizedName ?: @"", @"separate":@(NSScreen.screensHaveSeparateSpaces),@"fullscreen":@(fullscreenReady)};
                        puts([[NSString alloc] initWithData:[NSJSONSerialization dataWithJSONObject:snapshot options:0 error:nil] encoding:NSUTF8StringEncoding].UTF8String);
                        fflush(stdout);
                    } else if ([command hasPrefix:@"screen "]) {
                        NSArray *args=[command componentsSeparatedByString:@" "];
                        NSUInteger n=[args.lastObject integerValue];
                        if(n>=NSScreen.screens.count) {puts("missing display"); fflush(stdout); return;}
                        NSRect r=NSScreen.screens[n].visibleFrame;
                        NSWindow *w=[args[1] isEqual:@"A"]?a:b;
                        [w setFrameTopLeftPoint:NSMakePoint(NSMinX(r)+80,NSMaxY(r)-85)];
                        puts("moved"); fflush(stdout);
                    } else if ([command isEqual:@"fullscreen"]) {
                        a.collectionBehavior=NSWindowCollectionBehaviorFullScreenPrimary;
                        [a toggleFullScreen:nil]; puts("toggled"); fflush(stdout);
                    } else if ([command isEqual:@"host-space"]) {
                        puts(a.onActiveSpace ? "here" : "away"); fflush(stdout);
                    } else if ([command isEqual:@"type-digit"]) {
                        // Send an ordinary hardware digit only while one of the two test processes owns focus.
                        pid_t front = NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier;
                        if (front != jot_pid && front != getpid()) { puts("unrelated focus"); fflush(stdout); return; }
                        for (int down = 1; down >= 0; --down) {
                            CGEventRef e = CGEventCreateKeyboardEvent(NULL, kVK_ANSI_2, down);
                            CGEventSetFlags(e, 0);
                            CGEventPost(kCGHIDEventTap, e);
                            CFRelease(e);
                        }
                        puts("posted"); fflush(stdout);
                    } else if ([command hasPrefix:@"key "]) {
                        NSArray *args=[command componentsSeparatedByString:@" "];
                        if(NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier!=jot_pid){puts("not focused");fflush(stdout);return;}
                        CGKeyCode code=[args[1] intValue];
                        for(int down=1;down>=0;--down){
                            CGEventRef e=CGEventCreateKeyboardEvent(NULL,code,down);
                            CGEventSetFlags(e,(CGEventFlags)[args[2] longLongValue]); CGEventPostToPid(jot_pid,e); CFRelease(e);
                        }
                        puts("posted");fflush(stdout);
                    } else if ([command hasPrefix:@"click-host "]) {
                        puts(click_host([command hasSuffix:@"A"] ? a : b));
                        fflush(stdout);
                    } else if ([command hasPrefix:@"shift-host "]) {
                        NSWindow *target = [command hasSuffix:@"A"] ? a : b;
                        NSPoint original = target.frame.origin;
                        [target setFrameOrigin:NSMakePoint(original.x + 36, original.y + 18)];
                        puts("host moved"); fflush(stdout);
                    } else if ([command hasPrefix:@"expose "] || [command hasPrefix:@"expose-away "]) {
                        NSWindow *target = [command hasSuffix:@"A"] ? a : b;
                        target.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary;
                        NSRect area = NSScreen.screens.firstObject.visibleFrame;
                        CGPoint origin = CGPointMake(NSMaxX(area) - target.frame.size.width, NSMinY(area));
                        double x, y;
                        if (sscanf(command.UTF8String, "expose-away %lf %lf", &x, &y) == 2) {
                            CGFloat appKitY = NSMaxY(NSScreen.screens.firstObject.frame) - y;
                            origin.x = x > NSMidX(area) ? NSMinX(area) : NSMaxX(area) - target.frame.size.width;
                            origin.y = appKitY > NSMidY(area) ? NSMinY(area) : NSMaxY(area) - target.frame.size.height;
                        }
                        [target setFrameOrigin:origin];
                        [target orderFrontRegardless];
                        printf("exposed\n"); fflush(stdout);
                    } else if ([command hasPrefix:@"movecard "]) {
                        double x, y;
                        const char *result = sscanf(command.UTF8String, "movecard %lf %lf", &x, &y) == 2 ?
                            drag_file(jot_pid, CGPointMake(x, y), nil, NO) : "invalid command";
                        printf("drag %s\n", result);
                        fflush(stdout);
                    } else if ([command hasPrefix:@"drag "] || [command hasPrefix:@"switchdrag "]) {
                        double x, y;
                        char target;
                        const char *result = sscanf(command.UTF8String, "%*s %lf %lf %c", &x, &y, &target) == 3 ?
                            drag_file(jot_pid, CGPointMake(x, y), target == 'A' ? a : b, [command hasPrefix:@"switchdrag "]) : "invalid command";
                        printf("drag %s\n", result);
                        fflush(stdout);
                    } else if ([command isEqualToString:@"status"]) {
                        printf("status %d %s\n", NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier,
                            app.keyWindow.title.UTF8String ?: "");
                        fflush(stdout);
                    } else if ([command isEqualToString:@"note-stack"] || [command hasPrefix:@"note-stack "]) {
                        pid_t note_pid = [command hasPrefix:@"note-stack "] ? [command componentsSeparatedByString:@" "].lastObject.intValue : jot_pid;
                        NSArray *ordered = CFBridgingRelease(CGWindowListCopyWindowInfo(
                            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
                        NSMutableArray *stack = [NSMutableArray new];
                        for (NSDictionary *window in ordered) {
                            NSInteger pid = [window[(__bridge NSString *)kCGWindowOwnerPID] integerValue];
                            NSInteger number = [window[(__bridge NSString *)kCGWindowNumber] integerValue];
                            if (number == a.windowNumber) [stack addObject:@"A"];
                            else if (number == b.windowNumber) [stack addObject:@"B"];
                            else if (pid == note_pid) {
                                NSString *title = window[(__bridge NSString *)kCGWindowName];
                                if ([title isEqual:@"Jotiva"] || [title isEqual:@"Jotiva 便签"]) [stack addObject:title];
                            }
                        }
                        puts([stack componentsJoinedByString:@","].UTF8String);
                        fflush(stdout);
                    } else if ([command isEqualToString:@"front-host"]) {
                        NSArray *onScreen = CFBridgingRelease(CGWindowListCopyWindowInfo(
                            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
                        NSInteger hostNumber = -1, hostPid = -1;
                        for (NSDictionary *window in onScreen) {
                            if ([window[(__bridge NSString *)kCGWindowLayer] intValue] != 0 ||
                                [window[(__bridge NSString *)kCGWindowOwnerPID] intValue] == jot_pid) continue;
                            hostNumber = [window[(__bridge NSString *)kCGWindowNumber] integerValue];
                            hostPid = [window[(__bridge NSString *)kCGWindowOwnerPID] integerValue];
                            break;
                        }
                        if (hostNumber == a.windowNumber || hostNumber == b.windowNumber) puts(hostNumber == a.windowNumber ? "A" : "B");
                        else printf("%ld %ld\n", (long)hostPid, (long)hostNumber);
                        fflush(stdout);
                    } else if ([command hasPrefix:@"click-note"] || [command isEqualToString:@"geometry"] || [command isEqualToString:@"resize"]) {
                        CGRect frame;
                        if (!quick_frame(jot_pid, &frame) || !quick_on_top_at_center(jot_pid, frame)) puts("no quick");
                        else if ([command hasPrefix:@"click-note"]) {
                            // The folded window has an expand button at the trailing end of its title bar.
                            CGPoint point = frame.size.height <= 120 ? CGPointMake(CGRectGetMaxX(frame)-20, CGRectGetMinY(frame)+19)
                                : CGPointMake(CGRectGetMidX(frame), CGRectGetMidY(frame));
                            NSArray *args = [command componentsSeparatedByString:@" "];
                            if (args.count == 3) point = CGPointMake(CGRectGetMaxX(frame)-[args[1] doubleValue], CGRectGetMinY(frame)+[args[2] doubleValue]);
                            CGRect pixel = CGRectMake(point.x-.5, point.y-.5, 1, 1);
                            double sent = NSDate.date.timeIntervalSince1970 * 1000;
                            printf("%s %.3f\n", CGRectContainsPoint(frame, point) && quick_on_top_at_center(jot_pid, pixel) ? click_point(point) : "note occluded", sent);
                        } else if ([command isEqualToString:@"resize"]) { resize_quick(frame); puts("resize sent"); }
                        else printf("frame %.0f %.0f %.0f %.0f\n", frame.origin.x, frame.origin.y, frame.size.width, frame.size.height);
                        fflush(stdout);
                    } else if ([command isEqualToString:@"menu-close"]) {
                        puts(close_command(jot_pid).UTF8String);
                        fflush(stdout);
                    } else if ([command isEqualToString:@"quick-window"]) {
                        AXUIElementRef window = copy_quick_window(jot_pid);
                        puts(window ? "quick present" : "quick closed");
                        if (window) CFRelease(window);
                        fflush(stdout);
                    } else if ([command isEqualToString:@"native-close"]) {
                        double sent = NSDate.date.timeIntervalSince1970 * 1000;
                        AXError pressed = click_quick_close(jot_pid);
                        printf("native close %d %.3f\n", pressed, sent);
                        fflush(stdout);
                    } else if ([@[@"s", @"q", @"w", @"k", @"return"] containsObject:command]) {
                        bool quit = [command isEqualToString:@"q"];
                        bool close = [command isEqualToString:@"w"];
                        bool find = [command isEqualToString:@"k"];
                        bool enter = [command isEqualToString:@"return"];
                        bool quick = [command isEqualToString:@"s"];
                        pid_t expected = quick ? getpid() : jot_pid;
                        // A nonactivating note can own the keyboard while the host remains frontmost.
                        // Deliver Quit to our test process so a focus change cannot quit another app.
                        pid_t front = NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier;
                        if ((!quit && front != expected && !(quick && front == jot_pid)) || !CGPreflightPostEventAccess()) {
                            printf("key ignored\n");
                            fflush(stdout);
                            return;
                        }
                        if (close) fprintf(stderr, "Before Cmd+W: %s\n", close_command(jot_pid).UTF8String);
                        CGKeyCode code = quit ? kVK_ANSI_Q : close ? kVK_ANSI_W : find ? kVK_ANSI_K : enter ? kVK_Return : quick_key;
                        CGEventRef down = CGEventCreateKeyboardEvent(NULL, code, true);
                        CGEventRef up = CGEventCreateKeyboardEvent(NULL, code, false);
                        CGEventFlags mods = enter ? 0 : quick ? kCGEventFlagMaskControl | kCGEventFlagMaskShift : kCGEventFlagMaskCommand;
                        CGEventSetFlags(down, mods);
                        CGEventSetFlags(up, mods);
                        double sent = NSDate.date.timeIntervalSince1970 * 1000;
                        if (quit) {
                            CGEventPostToPid(jot_pid, down);
                            CGEventPostToPid(jot_pid, up);
                        } else {
                            CGEventPost(kCGSessionEventTap, down);
                            CGEventPost(kCGSessionEventTap, up);
                        }
                        CFRelease(down);
                        CFRelease(up);
                        printf("key posted %.3f\n", sent);
                        fflush(stdout);
                    }
                });
            }
            dispatch_async(dispatch_get_main_queue(), ^{ [app terminate:nil]; });
        });
        [app run];
    }
}
