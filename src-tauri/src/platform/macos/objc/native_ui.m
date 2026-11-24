#import <AppKit/AppKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>

@interface JotMenuSelection : NSObject
@property(nonatomic) NSInteger index;
@end

@implementation JotMenuSelection
- (void)choose:(NSMenuItem *)sender { self.index = sender.tag; }
@end

NSInteger jot_popup_menu(void *pointer, const char *const *labels, size_t count, double x, double y) {
    NSWindow *window = (__bridge NSWindow *)pointer;
    NSView *view = window.contentView;
    JotMenuSelection *selection = [JotMenuSelection new];
    selection.index = -1;
    NSMenu *menu = [NSMenu new];
    for (size_t i = 0; i < count; i++) {
        if (!labels[i]) {
            [menu addItem:NSMenuItem.separatorItem];
            continue;
        }
        NSMenuItem *item = [menu addItemWithTitle:[NSString stringWithUTF8String:labels[i]]
                                           action:@selector(choose:) keyEquivalent:@""];
        item.tag = i;
        item.target = selection;
    }
    NSPoint point = NSMakePoint(x, view.isFlipped ? y : NSHeight(view.bounds) - y);
    [menu popUpMenuPositioningItem:nil atLocation:point inView:view];
    return selection.index;
}

bool jot_copy_text(const char *text) {
    NSPasteboard *clipboard = NSPasteboard.generalPasteboard;
    [clipboard clearContents];
    return [clipboard setString:[NSString stringWithUTF8String:text] forType:NSPasteboardTypeString];
}

// Called on the AppKit event loop for the single File > Close command.
bool jot_close_key_window(void *tabbedWorkspace) {
    NSWindow *key = NSApp.keyWindow;
    if (key && key == (__bridge NSWindow *)tabbedWorkspace && !key.attachedSheet && !NSApp.modalWindow) {
        return true;
    }
    [NSApp sendAction:@selector(performClose:) to:nil from:nil];
    return false;
}

double jot_system_font_size(void) {
    return [NSFont preferredFontForTextStyle:NSFontTextStyleBody options:@{}].pointSize;
}

static NSMapTable<NSWindow *, NSNumber *> *quickWindows;
// AX and Rust placement use global top-left points. Commit frame and display together in AppKit points.
static void arrangeDocuments(NSWindow *left, NSWindow *right, NSScreen *screen) {
    for (NSWindow *window in @[left,right]) {
        if (window.styleMask & NSWindowStyleMaskFullScreen) {
            __block id observer = [NSNotificationCenter.defaultCenter addObserverForName:NSWindowDidExitFullScreenNotification
                object:window queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *event) {
                    (void)event;
                    [NSNotificationCenter.defaultCenter removeObserver:observer];
                    if (left.isVisible && right.isVisible) arrangeDocuments(left, right, screen);
                }];
            [window toggleFullScreen:nil];
            return;
        }
    }
    NSRect frame = screen.visibleFrame, a, b;
    NSDivideRect(frame, &a, &b, floor(NSWidth(frame)/2), NSMinXEdge);
    [left setFrame:a display:YES];
    [right setFrame:b display:YES];
    [left orderFront:nil];
    [right makeKeyAndOrderFront:nil];
}

void jot_tile_documents(void *caller, void *left, void *right) {
    arrangeDocuments((__bridge NSWindow *)left, (__bridge NSWindow *)right, ((__bridge NSWindow *)caller).screen ?: NSScreen.mainScreen);
}

static __weak NSWindow *requestedNote;

bool jot_activating_note(void) { return requestedNote != nil; }

static void completeNoteFocus(void) {
    NSWindow *note = requestedNote;
    requestedNote = nil;
    if (note.visible && note.onActiveSpace && NSApp.active) [note makeKeyAndOrderFront:nil];
}

void jot_focus_note(void *pointer, bool moveToHostSpace) {
    NSWindow *window = (__bridge NSWindow *)pointer;
    [window orderFrontRegardless];
    requestedNote = window;
    if (NSApp.active) {
        completeNoteFocus();
    } else {
        [window makeMainWindow];
        [window makeKeyWindow];
        if (moveToHostSpace) {
            [NSApp activateIgnoringOtherApps:YES];
        } else {
            [NSApp activate];
        }
    }
}

void jot_note_spaces(void *pointer, bool followsSpaces) {
    NSWindow *window = (__bridge NSWindow *)pointer;
    NSWindowCollectionBehavior behavior = window.collectionBehavior &
        ~(NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorMoveToActiveSpace |
          NSWindowCollectionBehaviorFullScreenPrimary | NSWindowCollectionBehaviorFullScreenNone |
          NSWindowCollectionBehaviorPrimary | NSWindowCollectionBehaviorAuxiliary | NSWindowCollectionBehaviorCanJoinAllApplications);
    behavior |= NSWindowCollectionBehaviorFullScreenAuxiliary;
    if (followsSpaces) behavior |= NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorCanJoinAllApplications;
    if (window.collectionBehavior != behavior) window.collectionBehavior = behavior;
}

void jot_place_note(void *pointer, double x, double y, double width, double height, bool followsSpaces) {
    NSWindow *window = (__bridge NSWindow *)pointer;
    jot_note_spaces(pointer, followsSpaces);
    NSRect frame = [window frameRectForContentRect:NSMakeRect(0, 0, width, height)];
    frame.origin = NSMakePoint(x, NSMaxY(NSScreen.screens.firstObject.frame) - y - NSHeight(frame));
    if (!NSEqualRects(window.frame, frame)) [window setFrame:frame display:YES animate:NO];
}

void jot_configure_quick_titlebar(void *pointer, uint64_t note, void (*resized)(uint64_t, double, double, double, double)) {
    NSWindow *window = (__bridge NSWindow *)pointer;
    [window standardWindowButton:NSWindowMiniaturizeButton].hidden = YES;
    [window standardWindowButton:NSWindowZoomButton].hidden = YES;
    window.tabbingMode = NSWindowTabbingModeDisallowed;
    window.titlebarSeparatorStyle = NSTitlebarSeparatorStyleNone;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        void (^settled)(NSNotification *) = ^(NSNotification *event) {
            (void)event;
            if (requestedNote) dispatch_async(dispatch_get_main_queue(), ^{ completeNoteFocus(); });
        };
        [NSNotificationCenter.defaultCenter addObserverForName:NSApplicationDidBecomeActiveNotification object:NSApp queue:nil usingBlock:settled];
        [NSNotificationCenter.defaultCenter addObserverForName:NSApplicationDidResignActiveNotification object:NSApp queue:nil usingBlock:^(NSNotification *event) {
            (void)event;
            requestedNote = nil;
        }];
        quickWindows = [NSMapTable weakToStrongObjectsMapTable];
        [NSNotificationCenter.defaultCenter addObserverForName:NSWindowDidEndLiveResizeNotification object:nil
            queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *event) {
                NSWindow *current = event.object;
                NSNumber *windowNote = [quickWindows objectForKey:current];
                if (!windowNote) return;
                NSRect frame = current.frame;
                NSSize size = current.contentView.bounds.size;
                resized(windowNote.unsignedLongLongValue, frame.origin.x, NSMaxY(NSScreen.screens.firstObject.frame) - NSMaxY(frame), size.width, size.height);
            }];
    });
    [quickWindows setObject:@(note) forKey:window];
}

// WebKit's HTML drop supplies names but intentionally hides absolute paths.
// Read the corresponding file URLs from this drag, leaving the general clipboard alone.
char *jot_dropped_file_paths(void) {
    NSPasteboard *board = [NSPasteboard pasteboardWithName:NSPasteboardNameDrag];
    NSArray<NSURL *> *files = [board readObjectsForClasses:@[NSURL.class]
        options:@{NSPasteboardURLReadingFileURLsOnlyKey: @YES}];
    NSMutableArray<NSString *> *paths = [NSMutableArray array];
    for (NSURL *file in files) {
        NSString *path = file.filePathURL.path;
        if (path) [paths addObject:path];
    }
    NSData *data = [NSJSONSerialization dataWithJSONObject:paths options:0 error:nil];
    return strdup([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String);
}

char *jot_choose_folder(void) {
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.canChooseFiles = NO;
    panel.canChooseDirectories = YES;
    panel.allowsMultipleSelection = NO;
    panel.canCreateDirectories = YES;
    panel.prompt = @"选择";
    if ([panel runModal] != NSModalResponseOK) return NULL;
    return strdup(panel.URL.fileSystemRepresentation);
}

// AppKit's field editor normally handles Escape by restoring the old text.
// Use the dialog's cancel/create actions for these two text commands.
@interface JotDocumentNameField : NSTextField <NSTextFieldDelegate>
@property (weak) NSAlert *prompt;
@end

@implementation JotDocumentNameField
- (BOOL)control:(NSControl *)control textView:(NSTextView *)editor doCommandBySelector:(SEL)command {
    if (editor.hasMarkedText) return NO;
    if (command == @selector(cancelOperation:)) {
        [self.prompt.buttons.lastObject performClick:nil];
        return YES;
    }
    if (command == @selector(insertNewline:)) {
        [self.prompt.buttons.firstObject performClick:nil];
        return YES;
    }
    return NO;
}
@end

char *jot_new_document_name(void) {
    NSAlert *alert = [NSAlert new];
    alert.messageText = @"新建文档";
    alert.informativeText = @"保存到「其他」";
    [alert addButtonWithTitle:@"创建"].keyEquivalent = @"\r";
    [alert addButtonWithTitle:@"取消"].keyEquivalent = @"\e";
    JotDocumentNameField *name = [JotDocumentNameField textFieldWithString:@"未命名.md"];
    name.prompt = alert;
    name.delegate = name;
    name.frame = NSMakeRect(0, 0, 260, name.fittingSize.height);
    name.accessibilityLabel = @"新文档名称";
    alert.accessoryView = name;
    alert.window.initialFirstResponder = name;
    CFRunLoopPerformBlock(CFRunLoopGetCurrent(), (__bridge CFStringRef)NSModalPanelRunLoopMode, ^{
        [alert.window makeFirstResponder:name];
        [name.currentEditor setSelectedRange:[name.stringValue rangeOfString:@"未命名"]];
    });
    if ([alert runModal] != NSAlertFirstButtonReturn) return NULL;
    return strdup((name.currentEditor.string ?: name.stringValue).UTF8String);
}

char *jot_choose_markdown(void) {
    NSOpenPanel *panel = [NSOpenPanel openPanel];
    panel.allowedContentTypes = @[[UTType typeWithFilenameExtension:@"md"], [UTType typeWithFilenameExtension:@"markdown"], [UTType typeWithFilenameExtension:@"txt"]];
    panel.canChooseDirectories = NO;
    if ([panel runModal] != NSModalResponseOK) return NULL;
    return strdup(panel.URL.fileSystemRepresentation);
}

void jot_file_error(const char *message) {
    NSAlert *alert = [[NSAlert alloc] init];
    alert.messageText = @"无法打开文档";
    alert.informativeText = [NSString stringWithUTF8String:message];
    [alert runModal];
}

char *jot_manage_path(int action, const char *source, const char *target, char **trashed) {
    @autoreleasepool {
        NSFileManager *manager = NSFileManager.defaultManager;
        NSURL *from = [NSURL fileURLWithFileSystemRepresentation:source isDirectory:NO relativeToURL:nil];
        NSURL *to = target ? [NSURL fileURLWithFileSystemRepresentation:target isDirectory:NO relativeToURL:nil] : nil;
        NSError *error = nil;
        BOOL ok;
        if (action == 0) {
            ok = [manager moveItemAtURL:from toURL:to error:&error];
        } else if (action == 1) {
            ok = [manager copyItemAtURL:from toURL:to error:&error];
        } else {
            NSURL *result = nil;
            ok = [manager trashItemAtURL:from resultingItemURL:&result error:&error];
            if (ok) *trashed = strdup(result.fileSystemRepresentation);
        }
        return ok ? NULL : strdup(error.localizedDescription.UTF8String);
    }
}

void jot_reveal_path(const char *path) {
    NSURL *url = [NSURL fileURLWithFileSystemRepresentation:path isDirectory:NO relativeToURL:nil];
    [NSWorkspace.sharedWorkspace activateFileViewerSelectingURLs:@[url]];
}
