#import "identity.h"
#import <CommonCrypto/CommonDigest.h>
#include <libproc.h>

NSString *JotApplicationKey(NSRunningApplication *app) {
    return app.bundleIdentifier ?: app.executableURL.path;
}

NSArray *JotSystemWindows(CGWindowListOption options) {
    return CFBridgingRelease(CGWindowListCopyWindowInfo(options | kCGWindowListExcludeDesktopElements, kCGNullWindowID));
}

static NSString *digest(NSString *value) {
    NSData *data = [value dataUsingEncoding:NSUTF8StringEncoding];
    unsigned char bytes[CC_SHA256_DIGEST_LENGTH];
    CC_SHA256(data.bytes, (CC_LONG)data.length, bytes);
    return [[NSData dataWithBytes:bytes length:sizeof(bytes)] base64EncodedStringWithOptions:0];
}

NSArray<NSDictionary *> *JotWindowIdentities(NSRunningApplication *app, NSArray *cgWindows) {
    NSString *appKey = JotApplicationKey(app);
    if (!appKey || app.terminated) return @[];
    AXUIElementRef application = AXUIElementCreateApplication(app.processIdentifier);
    AXUIElementSetMessagingTimeout(application, 0.06f);
    CFTypeRef raw = NULL;
    AXUIElementCopyAttributeValue(application, kAXWindowsAttribute, &raw);
    CFRelease(application);
    NSArray *windows = CFBridgingRelease(raw);
    if (![windows isKindOfClass:NSArray.class]) return @[];
    NSMutableArray *rows = [NSMutableArray array];
    NSCountedSet *frames = [NSCountedSet set], *documents = [NSCountedSet set];
    NSArray *attributes = @[(__bridge NSString *)kAXTitleAttribute, (__bridge NSString *)kAXPositionAttribute,
        (__bridge NSString *)kAXSizeAttribute, (__bridge NSString *)kAXDocumentAttribute,
        (__bridge NSString *)kAXMinimizedAttribute];
    for (id handle in windows) {
        AXUIElementRef window = (__bridge AXUIElementRef)handle;
        AXUIElementSetMessagingTimeout(window, 0.06f);
        CFArrayRef copied = NULL;
        AXUIElementCopyMultipleAttributeValues(window, (__bridge CFArrayRef)attributes, 0, &copied);
        NSArray *values = CFBridgingRelease(copied);
        if (values.count != attributes.count) return @[]; // Missing siblings make a match ambiguous.
        NSString *title = [values[0] isKindOfClass:NSString.class] ? values[0] : @"";
        NSMutableDictionary *row = [@{@"application": app, @"window": handle,
            @"source": title.length ? [NSString stringWithFormat:@"%@ · %@", app.localizedName ?: appKey, title] : (app.localizedName ?: appKey)} mutableCopy];
        row[@"minimized"] = @((__bridge CFTypeRef)values[4] == kCFBooleanTrue);
        CGPoint position;
        CGSize size;
        if (CFGetTypeID((__bridge CFTypeRef)values[1]) == AXValueGetTypeID()
            && CFGetTypeID((__bridge CFTypeRef)values[2]) == AXValueGetTypeID()
            && AXValueGetValue((__bridge AXValueRef)values[1], kAXValueCGPointType, &position)
            && AXValueGetValue((__bridge AXValueRef)values[2], kAXValueCGSizeType, &size)) {
            NSValue *frame = [NSValue valueWithRect:(NSRect){position, size}];
            [frames addObject:frame];
            row[@"frame"] = frame;
        }
        if ([values[3] isKindOfClass:NSString.class] && [values[3] length]) {
            row[@"document"] = digest(values[3]);
            [documents addObject:row[@"document"]];
        }
        [rows addObject:row];
    }
    NSMutableDictionary *numbers = [NSMutableDictionary dictionary];
    NSMutableDictionary *visible = [NSMutableDictionary dictionary];
    for (NSDictionary *cg in cgWindows) {
        if ([cg[(__bridge NSString *)kCGWindowOwnerPID] intValue] != app.processIdentifier ||
            [cg[(__bridge NSString *)kCGWindowLayer] intValue] != 0) continue;
        CGRect rect;
        if (!CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)cg[(__bridge NSString *)kCGWindowBounds], &rect)) continue;
        NSValue *frame = [NSValue valueWithRect:rect];
        NSNumber *number = cg[(__bridge NSString *)kCGWindowNumber];
        if ([cg[(__bridge NSString *)kCGWindowIsOnscreen] boolValue]) visible[frame] = visible[frame] ? NSNull.null : number;
        numbers[frame] = numbers[frame] ? NSNull.null : number;
    }
    struct proc_bsdinfo process = {0};
    NSString *instance = nil;
    if (proc_pidinfo(app.processIdentifier, PROC_PIDTBSDINFO, 0, &process, sizeof(process)) == sizeof(process)) {
        instance = [NSString stringWithFormat:@"%d/%llu.%06llu", app.processIdentifier,
            process.pbi_start_tvsec, process.pbi_start_tvusec];
    }
    NSMutableArray *eligible = [NSMutableArray array];
    for (NSMutableDictionary *row in rows) {
        NSValue *frame = row[@"frame"];
        if (![row[@"minimized"] boolValue] && !(frame && visible[frame])) continue;
        NSMutableDictionary *identity = [@{@"app": appKey} mutableCopy];
        if (instance) identity[@"instance"] = instance;
        NSString *document = row[@"document"];
        if (document) {
            identity[@"document"] = document;
            if ([documents countForObject:document] != 1) identity[@"ambiguous"] = @YES;
        }
        NSNumber *number = frame ? (visible[frame] ?: numbers[frame]) : nil;
        if ([number isKindOfClass:NSNumber.class] && [frames countForObject:frame] == 1) {
            row[@"cgId"] = number;
            if (instance) identity[@"session"] = [NSString stringWithFormat:@"%@/%@", instance, number];
        }
        row[@"identity"] = identity;
        [eligible addObject:row];
    }
    return eligible;
}

NSMutableDictionary *JotRecoveryMatches(NSDictionary *specs, NSArray *rows, NSMutableDictionary *initialWindows,
    NSMutableDictionary *createdWindows) {
    NSMutableDictionary *sessions = [NSMutableDictionary dictionary], *documents = [NSMutableDictionary dictionary];
    NSMutableDictionary *appWindows = [NSMutableDictionary dictionary];
    for (NSDictionary *row in rows) {
        NSDictionary *identity = row[@"identity"];
        NSString *app = identity[@"app"];
        if (!appWindows[app]) appWindows[app] = [NSMutableSet set];
        [appWindows[app] addObject:row[@"window"]];
        if (identity[@"session"]) {
            NSArray *key = @[app, identity[@"session"]];
            sessions[key] = sessions[key] ? NSNull.null : row;
        }
        if (identity[@"document"]) {
            NSArray *key = @[app, identity[@"document"]];
            documents[key] = documents[key] ? NSNull.null : row;
        }
    }
    NSMutableDictionary *matches = [NSMutableDictionary dictionary];
    for (NSString *origin in specs) {
        NSDictionary *wanted = specs[origin];
        NSString *app = wanted[@"app"];
        id exact = wanted[@"session"] ? sessions[@[app, wanted[@"session"]]] : nil;
        if ([exact isKindOfClass:NSDictionary.class] && (!wanted[@"document"] ||
                [exact[@"identity"][@"document"] isEqual:wanted[@"document"]])) {
            matches[origin] = exact;
        } else if (wanted[@"document"]) {
            id row = documents[@[app, wanted[@"document"]]];
            if ([row isKindOfClass:NSDictionary.class] && ![row[@"identity"][@"ambiguous"] boolValue]) {
                NSString *instance = row[@"identity"][@"instance"];
                NSRunningApplication *application = row[@"application"];
                BOOL relaunched = instance && wanted[@"instance"] && ![instance isEqual:wanted[@"instance"]];
                // The other same-document window may have existed before this source disappeared.
                BOOL newWindow = initialWindows[origin] && ![initialWindows[origin] containsObject:row[@"window"]];
                if (relaunched || newWindow || [createdWindows[@(application.processIdentifier)] containsObject:row[@"window"]]) {
                    matches[origin] = row;
                }
            }
        }
        if (!initialWindows[origin]) initialWindows[origin] = appWindows[app] ?: [NSSet set];
    }
    for (NSNumber *pid in createdWindows.allKeys) {
        NSRunningApplication *application = [NSRunningApplication runningApplicationWithProcessIdentifier:pid.intValue];
        NSString *key = JotApplicationKey(application);
        NSSet *windows = key ? appWindows[key] : nil;
        if (windows) [createdWindows[pid] intersectSet:windows];
        else [createdWindows removeObjectForKey:pid];
    }
    return matches;
}
