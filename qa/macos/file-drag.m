// A minimal external NSDraggingSource. It publishes the same file URL items as Finder.
#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#include <unistd.h>

@interface FileSource : NSView <NSDraggingSource>
@property (copy) NSArray<NSURL *> *files;
@end
@implementation FileSource
- (void)mouseDown:(NSEvent *)event {
    NSMutableArray *items = [NSMutableArray array];
    for (NSURL *file in self.files) {
        NSDraggingItem *item = [[NSDraggingItem alloc] initWithPasteboardWriter:file];
        [item setDraggingFrame:NSMakeRect(0, 0, 24, 24) contents:[NSImage imageNamed:NSImageNameMultipleDocuments]];
        [items addObject:item];
    }
    [self beginDraggingSessionWithItems:items event:event source:self];
}
- (NSDragOperation)draggingSession:(NSDraggingSession *)session sourceOperationMaskForDraggingContext:(NSDraggingContext)context {
    return NSDragOperationCopy;
}
- (void)draggingSession:(NSDraggingSession *)session endedAtPoint:(NSPoint)point operation:(NSDragOperation)operation {
    exit(operation == NSDragOperationNone ? 3 : 0);
}
@end

int main(int argc, char **argv) {
    if (argc < 4) return 1;
    @autoreleasepool {
        NSApplication *app = NSApplication.sharedApplication;
        [app setActivationPolicy:NSApplicationActivationPolicyAccessory];
        CGFloat top = NSMaxY(NSScreen.screens.firstObject.frame);
        CGPoint end = CGPointMake(atof(argv[1]), atof(argv[2]));
        CGPoint start = CGPointMake(end.x - 105, end.y);
        NSMutableArray *files = [NSMutableArray array];
        for (int i = 3; i < argc; i++) [files addObject:[NSURL fileURLWithPath:@(argv[i])]];
        NSPanel *panel = [[NSPanel alloc] initWithContentRect:NSMakeRect(start.x-25, top-start.y-25, 50, 50)
              styleMask:NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        FileSource *source = [FileSource new];
        source.files = files;
        panel.contentView = source;
        panel.level = NSFloatingWindowLevel;
        [panel orderFrontRegardless];
        [app finishLaunching];
        dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 300*NSEC_PER_MSEC), dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^{
            for (int i = -1; i <= 23; i++) {
                CGEventType type = i < 0 ? kCGEventMouseMoved : i == 0 ? kCGEventLeftMouseDown :
                    i == 23 ? kCGEventLeftMouseUp : kCGEventLeftMouseDragged;
                CGPoint p = CGPointMake(start.x + MAX(0, MIN(i,22)) * (end.x-start.x)/22., end.y);
                CGEventRef event = CGEventCreateMouseEvent(NULL, type, p, kCGMouseButtonLeft);
                CGEventSetFlags(event, 0);
                CGEventSetIntegerValueField(event, kCGMouseEventClickState, 1);
                CGEventPost(kCGHIDEventTap, event);
                CFRelease(event);
                usleep(24000);
            }
        });
        [app run];
    }
    return 1;
}
