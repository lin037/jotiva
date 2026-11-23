#import <AppKit/AppKit.h>

extern char *jot_new_document_name(void);

static void after(NSTimeInterval delay, void (^block)(void)) {
    NSTimer *timer = [NSTimer timerWithTimeInterval:delay repeats:NO block:^(NSTimer *timer) { block(); }];
    [[NSRunLoop currentRunLoop] addTimer:timer forMode:NSModalPanelRunLoopMode];
}

int main(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        [NSApp finishLaunching];
        for (NSNumber *accept in @[@NO, @YES]) {
            __block BOOL receivedKey = NO;
            after(.12, ^{
                NSWindow *window = NSApp.modalWindow;
                NSTextView *editor = (NSTextView *)window.firstResponder;
                assert([editor isKindOfClass:NSTextView.class]);
                assert([editor.string isEqualToString:@"未命名.md"]);
                assert(NSEqualRanges(editor.selectedRange, NSMakeRange(0, 3)));
                [editor insertText:@"评审纪要" replacementRange:editor.selectedRange];
                NSString *key = accept.boolValue ? @"\r" : @"\e";
                NSEvent *event = [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint
                    modifierFlags:0 timestamp:NSProcessInfo.processInfo.systemUptime
                    windowNumber:window.windowNumber context:nil characters:key charactersIgnoringModifiers:key
                    isARepeat:NO keyCode:(accept.boolValue ? 36 : 53)];
                receivedKey = YES;
                [editor interpretKeyEvents:@[event]];
            });
            char *name = jot_new_document_name();
            assert(receivedKey);
            if (accept.boolValue) {
                assert(name && strcmp(name, "评审纪要.md") == 0);
                free(name);
            } else assert(name == NULL);
        }
        puts("Selected filename stem, Escape cancelled, Return accepted the named Markdown.");
    }
}
