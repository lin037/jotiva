// Standalone AppKit regression. Link with src-tauri/src/platform/macos/objc/native_ui.m in a test executable.
#import <AppKit/AppKit.h>

extern NSInteger jot_popup_menu(void *window, const char *const *labels, size_t count, double x, double y);

int main(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        [NSApp finishLaunching];
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(100, 100, 240, 100)
                                                     styleMask:NSWindowStyleMaskTitled
                                                       backing:NSBackingStoreBuffered defer:NO];
        const char *labels[] = {"打开", NULL, "复制相对路径"};
        for (NSNumber *expected in @[@2, @(-1)]) {
            __block BOOL tracked = NO;
            id observer = [NSNotificationCenter.defaultCenter
                addObserverForName:NSMenuDidBeginTrackingNotification object:nil queue:nil
                usingBlock:^(NSNotification *notification) {
                    NSMenu *menu = notification.object;
                    tracked = menu.numberOfItems == 3 && [menu itemAtIndex:1].separatorItem
                        && [[menu itemAtIndex:2].title isEqualToString:@"复制相对路径"];
                    CFRunLoopPerformBlock(CFRunLoopGetCurrent(), (__bridge CFStringRef)NSEventTrackingRunLoopMode, ^{
                        if (expected.integerValue >= 0) [menu performActionForItemAtIndex:expected.integerValue];
                        [menu cancelTracking];
                    });
                }];
            NSInteger result = jot_popup_menu((__bridge void *)window, labels, 3, 20, 20);
            [NSNotificationCenter.defaultCenter removeObserver:observer];
            if (!tracked || result != expected.integerValue) return 1;
        }
        puts("Native context menu returned the chosen row and dismissed without an action on cancel.");
    }
    return 0;
}
