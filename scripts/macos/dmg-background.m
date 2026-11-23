// Draw the install window at Finder's logical size and at Retina resolution.
#import <AppKit/AppKit.h>

static NSColor *rgb(unsigned value) {
    return [NSColor colorWithSRGBRed:((value >> 16) & 255) / 255.0
                               green:((value >> 8) & 255) / 255.0
                                blue:(value & 255) / 255.0 alpha:1];
}

// A stack of pages lifts at the far right. Keep the icon and caption areas clear.
static NSBezierPath *page(CGFloat offset) {
    NSBezierPath *path = [NSBezierPath bezierPath];
    [path moveToPoint:NSMakePoint(-2, 320)];
    [path lineToPoint:NSMakePoint(578, 320)];
    [path lineToPoint:NSMakePoint(578, 196 - offset)];
    [path curveToPoint:NSMakePoint(496, 31 - offset)
         controlPoint1:NSMakePoint(565, 133 - offset) controlPoint2:NSMakePoint(540, 68 - offset)];
    [path curveToPoint:NSMakePoint(256, -7 - offset)
         controlPoint1:NSMakePoint(452, -5 - offset) controlPoint2:NSMakePoint(360, -4 - offset)];
    [path lineToPoint:NSMakePoint(-2, -12 - offset)];
    [path closePath];
    return path;
}

static void drawPaper(void) {
    // The little patch of cover uses the same teal as Jotiva's app icon.
    [rgb(0x17796e) setFill];
    NSRectFill(NSMakeRect(0, 0, 576, 318));
    for (NSNumber *distance in @[@40, @29, @19, @10, @0]) {
        NSUInteger i = distance.unsignedIntegerValue;
        NSBezierPath *sheet = page(i);
        NSColor *edge = i == 0 ? rgb(0xe9ede8) : rgb(0xb1c8bd);
        NSColor *face = i == 0 ? rgb(0xfcfbf7) : rgb(0xe6eee7);
        NSGradient *light = [[NSGradient alloc] initWithStartingColor:edge endingColor:face];
        [light drawInBezierPath:sheet relativeCenterPosition:NSMakePoint(1.1, -1.1)];
        [rgb(i == 0 ? 0xe1e6df : 0xa8beb5) setStroke];
        sheet.lineWidth = 0.65;
        [sheet stroke];
    }
}

static void caption(NSString *text, CGFloat centerY, CGFloat size,
                    NSFontWeight weight, unsigned color) {
    NSDictionary *attributes = @{
        NSFontAttributeName: [NSFont systemFontOfSize:size weight:weight],
        NSForegroundColorAttributeName: rgb(color)
    };
    NSSize bounds = [text sizeWithAttributes:attributes];
    [text drawAtPoint:NSMakePoint((576 - bounds.width) / 2, centerY - bounds.height / 2)
       withAttributes:attributes];
}

static void drawArrow(CGFloat x, CGFloat y) {
    NSBezierPath *arrow = [NSBezierPath bezierPath];
    [arrow moveToPoint:NSMakePoint(x - 16, y)];
    [arrow lineToPoint:NSMakePoint(x + 16, y)];
    [arrow moveToPoint:NSMakePoint(x + 8, y + 8)];
    [arrow lineToPoint:NSMakePoint(x + 16, y)];
    [arrow lineToPoint:NSMakePoint(x + 8, y - 8)];
    arrow.lineWidth = 2.0;
    arrow.lineCapStyle = NSLineCapStyleRound;
    arrow.lineJoinStyle = NSLineJoinStyleRound;
    [rgb(0x74908a) setStroke];
    [arrow stroke];
}

// Finder on Tahoe can briefly leave a hollow placeholder for a working symlink.
// This small outline sits entirely behind the opaque body of Finder's folder.
static void folderHint(CGFloat x, CGFloat y) {
    NSBezierPath *path = [NSBezierPath bezierPath];
    [path moveToPoint:NSMakePoint(x - 16, y - 11)];
    [path lineToPoint:NSMakePoint(x + 16, y - 11)];
    [path curveToPoint:NSMakePoint(x + 19, y - 8)
         controlPoint1:NSMakePoint(x + 18, y - 11) controlPoint2:NSMakePoint(x + 19, y - 10)];
    [path lineToPoint:NSMakePoint(x + 19, y + 6)];
    [path curveToPoint:NSMakePoint(x + 16, y + 9)
         controlPoint1:NSMakePoint(x + 19, y + 8) controlPoint2:NSMakePoint(x + 18, y + 9)];
    [path lineToPoint:NSMakePoint(x - 3, y + 9)];
    [path lineToPoint:NSMakePoint(x - 7, y + 13)];
    [path lineToPoint:NSMakePoint(x - 16, y + 13)];
    [path curveToPoint:NSMakePoint(x - 19, y + 10)
         controlPoint1:NSMakePoint(x - 18, y + 13) controlPoint2:NSMakePoint(x - 19, y + 12)];
    [path lineToPoint:NSMakePoint(x - 19, y - 8)];
    [path curveToPoint:NSMakePoint(x - 16, y - 11)
         controlPoint1:NSMakePoint(x - 19, y - 10) controlPoint2:NSMakePoint(x - 18, y - 11)];
    [path closePath];
    path.lineJoinStyle = NSLineJoinStyleRound;
    path.lineWidth = 1.8;
    [rgb(0x698a83) setStroke];
    [path stroke];
}

static BOOL render(NSString *directory, NSInteger scale, NSDictionary *layout, NSString *name) {
    CGFloat width = [layout[@"windowSize"][@"width"] doubleValue];
    CGFloat height = [layout[@"windowSize"][@"height"] doubleValue];
    CGFloat left = [layout[@"appPosition"][@"x"] doubleValue];
    CGFloat right = [layout[@"applicationFolderPosition"][@"x"] doubleValue];
    CGFloat iconY = height - [layout[@"applicationFolderPosition"][@"y"] doubleValue];
    NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc]
        initWithBitmapDataPlanes:NULL pixelsWide:width * scale pixelsHigh:height * scale
        bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO
        colorSpaceName:NSCalibratedRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
    bitmap.size = NSMakeSize(width, height);
    [NSGraphicsContext saveGraphicsState];
    [NSGraphicsContext setCurrentContext:[NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap]];
    drawPaper();
    drawArrow((left + right) / 2, iconY);
    folderHint(right, iconY);
    caption([NSString stringWithFormat:@"将 %@ 拖入「应用程序」", name],
            height - 202, 17, NSFontWeightMedium, 0x303b37);
    caption([NSString stringWithFormat:@"Drag %@ into Applications.", name],
            height - 225, 12, NSFontWeightRegular, 0x55635d);
    [NSGraphicsContext restoreGraphicsState];
    NSString *file = scale == 1 ? @"install.png" : @"install@2x.png";
    return [[bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
            writeToFile:[directory stringByAppendingPathComponent:file] atomically:YES];
}

int main(int argc, const char **argv) {
    @autoreleasepool {
        if (argc != 2) return 2;
        NSData *data = [NSData dataWithContentsOfFile:@"src-tauri/tauri.conf.json"];
        if (!data) return 1;
        NSDictionary *config = [NSJSONSerialization JSONObjectWithData:data options:0 error:NULL];
        NSDictionary *layout = config[@"bundle"][@"macOS"][@"dmg"];
        NSString *name = config[@"productName"];
        if (!layout || !name.length) return 1;
        NSString *directory = [NSString stringWithUTF8String:argv[1]];
        return render(directory, 1, layout, name) && render(directory, 2, layout, name) ? 0 : 1;
    }
}
