#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#import "identity.h"

typedef struct {
    double x, y, width, height;
} JotRect;

typedef struct {
    uint64_t id;
    pid_t pid;
    bool own_app;
    bool has_window;
    bool fullscreen;
    JotRect screen;
    char source[256];
} JotHost;

@interface JotObservedApp : NSObject {
@public
    AXObserverRef observer;
    AXUIElementRef app;
    NSMutableDictionary<NSNumber *, id> *windows;
    CFMutableDictionaryRef identities;
}
@end

@implementation JotObservedApp
- (void)dealloc {
    if (observer) {
        CFRunLoopRemoveSource(CFRunLoopGetMain(), AXObserverGetRunLoopSource(observer), kCFRunLoopCommonModes);
        CFRelease(observer);
    }
    if (app) CFRelease(app);
    if (identities) CFRelease(identities);
}
@end

static NSMutableDictionary<NSNumber *, JotObservedApp *> *observed;
static uint64_t next_id;
static void (*notify)(uint64_t, bool);
static bool space_changed;
static bool refresh_pending;
static NSMutableDictionary<NSNumber *, id> *pending_main_windows;
static NSDictionary<NSNumber *, id> *reported_main_windows;
@interface JotChoiceSnapshot : NSObject
@property uint64_t generation;
@property (strong) NSDictionary<NSNumber *, NSDictionary *> *handles;
@end
@implementation JotChoiceSnapshot
@end

static NSMutableDictionary<NSString *, JotChoiceSnapshot *> *source_snapshots;
static uint64_t source_generation;
static uint64_t next_choice;
static NSDictionary *drop_handle;
static NSNumber *drop_choice;
static id drag_escape_local, drag_escape_global;
static bool drag_in_progress, drag_cancelled;
static pid_t drag_target_pid;

void jot_drag_started(void) {
    drag_in_progress = true;
    drag_cancelled = false;
    drag_target_pid = 0;
    if (drag_escape_local) return;
    drag_escape_local = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:^NSEvent *(NSEvent *key) {
        if (key.keyCode == 53) drag_cancelled = true;
        return key;
    }];
    drag_escape_global = [NSEvent addGlobalMonitorForEventsMatchingMask:NSEventMaskKeyDown handler:^(NSEvent *key) {
        if (key.keyCode == 53) drag_cancelled = true;
    }];
}
void jot_drag_ended(void) {
    drag_in_progress = false;
    if (drag_escape_local) { [NSEvent removeMonitor:drag_escape_local]; drag_escape_local = nil; }
    if (drag_escape_global) { [NSEvent removeMonitor:drag_escape_global]; drag_escape_global = nil; }
}

static NSDictionary *recovery_specs;
static NSSet *recovery_apps;
static NSMutableDictionary<NSNumber *, NSMutableSet *> *reopened_windows;
static NSMutableDictionary<NSString *, NSSet *> *existing_windows;
static bool recovery_running, recovery_pending;
static uint64_t recovery_epoch;
static bool (*recover_host)(uint64_t, uint64_t, const char *);
static void schedule_recovery(void);
static bool settling_recovery;
static void recovery_after_window_opens(void) {
    if (!recovery_specs.count) return;
    schedule_recovery();
    if (settling_recovery) return;
    settling_recovery = true;
    // AX reports creation before AppKit publishes the new window geometry.
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, 250 * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{
        settling_recovery = false;
        schedule_recovery();
    });
}
void jot_host_unwatch(uint64_t);

static void request_refresh(void) {
    if (refresh_pending) return;
    refresh_pending = true;
    dispatch_async(dispatch_get_main_queue(), ^{
        refresh_pending = false;
        reported_main_windows = pending_main_windows;
        pending_main_windows = nil;
        bool changed = space_changed;
        space_changed = false;
        notify(0, changed);
        reported_main_windows = nil;
    });
}

// Workspace activation can precede a host publishing either AX window. Some
// hosts send no window notification at all when the same document stays main.
static pid_t activating_pid;
static unsigned activation_attempt;
static uint64_t activation_generation;
static bool activation_retry_pending;

static void begin_activation(pid_t pid) {
    ++activation_generation;
    JotObservedApp *observation = observed[@(pid)];
    activating_pid = observation && observation->windows.count ? pid : 0;
    activation_attempt = 0;
    activation_retry_pending = false;
}

static void retry_activation(pid_t pid) {
    static const unsigned delays_ms[] = {60, 240, 900};
    if (activating_pid != pid || activation_retry_pending ||
        activation_attempt >= sizeof(delays_ms) / sizeof(delays_ms[0])) return;
    uint64_t generation = activation_generation;
    unsigned delay = delays_ms[activation_attempt++];
    activation_retry_pending = true;
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, delay * NSEC_PER_MSEC), dispatch_get_main_queue(), ^{
        if (generation != activation_generation || activating_pid != pid) return;
        activation_retry_pending = false;
        JotObservedApp *observation = observed[@(pid)];
        if (observation && observation->windows.count && NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == pid)
            request_refresh();
    });
}

static NSNumber *window_id(JotObservedApp *observation, AXUIElementRef window) {
    return observation ? (__bridge NSNumber *)CFDictionaryGetValue(observation->identities, window) : nil;
}

static void ax_event(AXObserverRef observer, AXUIElementRef element, CFStringRef name, void *context) {
    (void)observer;
    NSNumber *pid = @((pid_t)(intptr_t)context);
    JotObservedApp *observation = observed[pid];
    if (!observation) return;
    if (CFEqual(name, kAXMovedNotification) || CFEqual(name, kAXResizedNotification)) {
        if (NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != pid.intValue) return;
        // Only resample the screen after the drag settles.
        static NSTimer *screenCheck;
        if (screenCheck) {
            screenCheck.fireDate = [NSDate dateWithTimeIntervalSinceNow:0.09];
        } else {
            screenCheck = [NSTimer scheduledTimerWithTimeInterval:0.09 repeats:NO block:^(NSTimer *timer) {
                (void)timer;
                screenCheck = nil;
                request_refresh();
            }];
        }
        return;
    }
    if (CFEqual(name, kAXUIElementDestroyedNotification)) {
        NSNumber *key = window_id(observation, element);
        if (!key) return;
        CFDictionaryRemoveValue(observation->identities, element);
        [observation->windows removeObjectForKey:key];
        uint64_t id = key.unsignedLongLongValue;
        dispatch_async(dispatch_get_main_queue(), ^{
            if (!observation->windows.count && observed[pid] == observation) [observed removeObjectForKey:pid];
            notify(id, false);
        });
    } else {
        NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:pid.intValue];
        NSString *appKey = JotApplicationKey(app);
        if (CFEqual(name, kAXWindowCreatedNotification) && appKey && [recovery_apps containsObject:appKey]) {
            if (!reopened_windows[pid]) reopened_windows[pid] = [NSMutableSet set];
            [reopened_windows[pid] addObject:(__bridge id)element];
        }
        bool mainChanged = CFEqual(name, kAXMainWindowChangedNotification);
        bool documentChanged = mainChanged || CFEqual(name, kAXFocusedWindowChangedNotification);
        if (mainChanged && observation->windows.count) {
            if (!pending_main_windows) pending_main_windows = [NSMutableDictionary dictionary];
            // AX supplies the new main window (the app element means no main window).
            pending_main_windows[pid] = CFEqual(element, observation->app) ? NSNull.null : (__bridge id)element;
        }
        if (appKey && [recovery_apps containsObject:appKey]) {
            if (CFEqual(name, kAXWindowCreatedNotification)) recovery_after_window_opens();
            else if (documentChanged) schedule_recovery();
        }
        // AX can announce the window before NSWorkspace delivers app activation.
        // Resolve the active owner in the coalesced refresh, after both notifications.
        if (documentChanged || NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier == pid.intValue)
            request_refresh();
    }
}

static bool read_value(CFTypeRef value, AXValueType type, void *out) {
    return CFGetTypeID(value) == AXValueGetTypeID() && AXValueGetValue(value, type, out);
}

bool jot_host_resigned(void) { return !NSApp.active; }

bool jot_host_permission(bool prompt) {
    if (!prompt) return AXIsProcessTrusted();
    NSDictionary *options = @{(__bridge NSString *)kAXTrustedCheckOptionPrompt: @YES};
    return AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)options);
}

bool jot_open_accessibility_settings(void) {
    NSURL *url = [NSURL URLWithString:@"x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"];
    return [NSWorkspace.sharedWorkspace openURL:url];
}

void jot_host_watch(void (*callback)(uint64_t, bool)) {
    notify = callback;
    observed = [NSMutableDictionary dictionary];
    [NSNotificationCenter.defaultCenter addObserverForName:NSApplicationDidResignActiveNotification
        object:NSApp queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *message) {
            (void)message;
            request_refresh();
        }];
    NSNotificationCenter *center = NSWorkspace.sharedWorkspace.notificationCenter;
    [center addObserverForName:NSWorkspaceActiveSpaceDidChangeNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *message) {
        (void)message;
        space_changed = true;
        begin_activation(NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier);
        request_refresh();
    }];
    [NSNotificationCenter.defaultCenter addObserverForName:NSApplicationDidChangeScreenParametersNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *message) {
        (void)message;
        request_refresh();
    }];
    [center addObserverForName:NSWorkspaceDidActivateApplicationNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *message) {
        NSRunningApplication *front = message.userInfo[NSWorkspaceApplicationKey];
        begin_activation(front.processIdentifier);
        if (drag_in_progress) drag_target_pid = front.processIdentifier;
        request_refresh();
        schedule_recovery();
    }];
    [center addObserverForName:NSWorkspaceDidLaunchApplicationNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *message) {
        (void)message;
        recovery_after_window_opens();
    }];
    [center addObserverForName:NSWorkspaceDidTerminateApplicationNotification object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *message) {
        NSRunningApplication *application = message.userInfo[NSWorkspaceApplicationKey];
        NSNumber *pid = @(application.processIdentifier);
        [reopened_windows removeObjectForKey:pid];
        JotObservedApp *observation = observed[pid];
        if (!observation) return;
        NSArray<NSNumber *> *keys = observation->windows.allKeys;
        [observed removeObjectForKey:pid];
        for (NSNumber *key in keys) notify(key.unsignedLongLongValue, false);
    }];
}

static JotObservedApp *observe_app(NSRunningApplication *front, AXUIElementRef application) {
    NSNumber *pid = @(front.processIdentifier);
    JotObservedApp *observation = observed[pid];
    bool new_app = observation == nil;
    void *context = (void *)(intptr_t)front.processIdentifier;
    if (new_app) {
        observation = [JotObservedApp new];
        observation->windows = [NSMutableDictionary dictionary];
        observation->identities = CFDictionaryCreateMutable(
            kCFAllocatorDefault, 0, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks
        );
        observation->app = (AXUIElementRef)CFRetain(application);
        if (AXObserverCreate(front.processIdentifier, ax_event, &observation->observer) != kAXErrorSuccess) return nil;
        AXError focus = AXObserverAddNotification(observation->observer, application, kAXFocusedWindowChangedNotification, context);
        AXError main = AXObserverAddNotification(observation->observer, application, kAXMainWindowChangedNotification, context);
        if (focus != kAXErrorSuccess && main != kAXErrorSuccess) return nil;
        AXObserverAddNotification(observation->observer, application, kAXWindowCreatedNotification, context);
        observed[pid] = observation;
        CFRunLoopAddSource(CFRunLoopGetMain(), AXObserverGetRunLoopSource(observation->observer), kCFRunLoopCommonModes);
    }
    return observation;
}

static NSNumber *watch_window(NSRunningApplication *front, AXUIElementRef application, AXUIElementRef window) {
    JotObservedApp *observation = observe_app(front, application);
    if (!observation) return nil;
    void *context = (void *)(intptr_t)front.processIdentifier;
    AXError result = AXObserverAddNotification(observation->observer, window, kAXUIElementDestroyedNotification, context);
    if (result != kAXErrorSuccess) return nil;
    CFStringRef events[] = {
        kAXWindowMiniaturizedNotification, kAXWindowDeminiaturizedNotification, kAXMovedNotification, kAXResizedNotification
    };
    for (size_t i = 0; i < sizeof(events) / sizeof(events[0]); ++i) {
        AXObserverAddNotification(observation->observer, window, events[i], context);
    }
    NSNumber *key = @(++next_id);
    observation->windows[key] = (__bridge id)window;
    CFDictionarySetValue(observation->identities, window, (__bridge CFTypeRef)key);
    return key;
}

// Reserve before dispatching the slow AX enumeration. Closing or refreshing that
// picker invalidates its pending lookup, even if the background job has not started.
uint64_t jot_begin_source_choices(const char *owner) {
    @synchronized (JotObservedApp.class) {
        if (!source_snapshots) source_snapshots = [NSMutableDictionary dictionary];
        JotChoiceSnapshot *snapshot = [JotChoiceSnapshot new];
        snapshot.generation = ++source_generation;
        source_snapshots[@(owner)] = snapshot;
        return snapshot.generation;
    }
}

static bool publish_choices(NSString *owner, uint64_t generation, NSMutableArray *items, NSArray *handles) {
    @synchronized (JotObservedApp.class) {
        JotChoiceSnapshot *snapshot = source_snapshots[owner];
        if (snapshot.generation != generation) return false;
        NSMutableDictionary *choices = [NSMutableDictionary dictionary];
        for (NSUInteger index = 0; index < items.count; ++index) {
            NSNumber *choice = @(++next_choice);
            NSMutableDictionary *item = [items[index] mutableCopy];
            item[@"choice"] = choice;
            items[index] = item;
            choices[choice] = handles[index];
        }
        snapshot.handles = choices;
        return true;
    }
}

static AXError append_source_windows(NSNumber *pid, float timeout, NSMutableArray *items, NSMutableArray *handles) {
    NSRunningApplication *application = [NSRunningApplication runningApplicationWithProcessIdentifier:pid.intValue];
    if (!application) return kAXErrorInvalidUIElement;
    AXUIElementRef element = AXUIElementCreateApplication(pid.intValue);
    AXUIElementSetMessagingTimeout(element, timeout);
    CFTypeRef raw = NULL;
    AXError result = AXUIElementCopyAttributeValue(element, kAXWindowsAttribute, &raw);
    CFRelease(element);
    if (raw && CFGetTypeID(raw) == CFArrayGetTypeID()) {
        NSString *app_name = application.localizedName ?: @"当前应用";
        for (id handle in (__bridge NSArray *)raw) {
            AXUIElementRef window = (__bridge AXUIElementRef)handle;
            AXUIElementSetMessagingTimeout(window, 0.04f);
            CFTypeRef copied = NULL;
            AXUIElementCopyAttributeValue(window, kAXTitleAttribute, &copied);
            NSString *title = copied && CFGetTypeID(copied) == CFStringGetTypeID() ? (__bridge NSString *)copied : @"";
            NSString *source = title.length ? [NSString stringWithFormat:@"%@ · %@", app_name, title] : app_name;
            [handles addObject:@{@"application": application, @"window": handle, @"source": source}];
            [items addObject:@{@"app": app_name, @"title": title}];
            if (copied) CFRelease(copied);
        }
    }
    if (raw) CFRelease(raw);
    return result;
}

char *jot_source_choices(const char *owner, uint64_t generation) {
    @autoreleasepool {
        NSMutableArray *items = [NSMutableArray array];
        NSMutableArray *handles = [NSMutableArray array];
        if (!AXIsProcessTrusted()) return NULL;
        NSMutableSet<NSNumber *> *pids = [NSMutableSet set];
        for (NSRunningApplication *application in NSWorkspace.sharedWorkspace.runningApplications) {
            if (application.activationPolicy == NSApplicationActivationPolicyRegular) {
                [pids addObject:@(application.processIdentifier)];
            }
        }
        // A recently launched window can be on screen before application registration settles.
        NSMutableSet<NSNumber *> *visible_pids = [NSMutableSet set];
        CFArrayRef window_list = CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID
        );
        for (NSDictionary *window in (__bridge NSArray *)window_list) {
            if ([window[(__bridge NSString *)kCGWindowLayer] integerValue] == 0) {
                [visible_pids addObject:window[(__bridge NSString *)kCGWindowOwnerPID]];
            }
        }
        if (window_list) CFRelease(window_list);
        [pids unionSet:visible_pids];
        [pids removeObject:@(getpid())];
        NSMutableArray<NSNumber *> *slow = [NSMutableArray array];
        for (NSNumber *pid in pids) {
            AXError result = append_source_windows(pid, 0.04f, items, handles);
            if (result == kAXErrorCannotComplete && [visible_pids containsObject:pid]) [slow addObject:pid];
        }
        // A visible app can still be bringing up its AX tree on the first query.
        // Give only those timed-out owners one more chance within this same scan.
        for (NSNumber *pid in slow) {
            @synchronized (JotObservedApp.class) {
                if (source_snapshots[@(owner)].generation != generation) return strdup("[]");
            }
            append_source_windows(pid, 0.16f, items, handles);
        }
        if (!publish_choices(@(owner), generation, items, handles)) return strdup("[]");
        NSData *json = [NSJSONSerialization dataWithJSONObject:items options:0 error:nil];
        return strdup([[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding].UTF8String);
    }
}

void jot_clear_source_choices(const char *owner) {
    @synchronized (JotObservedApp.class) {
        [source_snapshots removeObjectForKey:@(owner)];
    }
}

static JotHost register_entry(NSDictionary *entry) {
    JotHost result = {0};
    if (!entry || !AXIsProcessTrusted()) return result;
    NSRunningApplication *application = entry[@"application"];
    if (application.terminated) return result;
    pid_t pid = application.processIdentifier;
    AXUIElementRef element = AXUIElementCreateApplication(pid);
    AXUIElementSetMessagingTimeout(element, 0.05f);
    AXUIElementRef window = (__bridge AXUIElementRef)entry[@"window"];
    NSNumber *id = window_id(observed[@(pid)], window);
    if (!id) id = watch_window(application, element, window);
    CFRelease(element);
    if (id) {
        result.id = id.unsignedLongLongValue;
        result.pid = pid;
        strlcpy(result.source, [entry[@"source"] UTF8String], sizeof(result.source));
    }
    return result;
}

static NSDictionary *entry_for_choice(uint64_t choice) {
    NSNumber *key = @(choice);
    @synchronized (JotObservedApp.class) {
        if ([drop_choice isEqual:key]) return drop_handle;
        for (JotChoiceSnapshot *snapshot in source_snapshots.objectEnumerator) {
            NSDictionary *entry = snapshot.handles[key];
            if (entry) return entry;
        }
    }
    return nil;
}

JotHost jot_register_source(uint64_t choice) {
    return register_entry(entry_for_choice(choice));
}

static int64_t issue_drop_choice(NSDictionary *row) {
    @synchronized (JotObservedApp.class) {
        drop_choice = @(++next_choice);
        drop_handle = row;
        return drop_choice.longLongValue;
    }
}

// An inspector or keyboard panel can be key while its document stays main.
// A specifically attached key window wins; otherwise keep that document's host.
static id active_window(AXUIElementRef application, JotObservedApp *observation, id reportedMain) {
    NSArray *attributes = @[(__bridge NSString *)kAXFocusedWindowAttribute, (__bridge NSString *)kAXMainWindowAttribute];
    CFArrayRef copied = NULL;
    AXUIElementCopyMultipleAttributeValues(application, (__bridge CFArrayRef)attributes, 0, &copied);
    NSArray *windows = CFBridgingRelease(copied);
    id focused = windows.firstObject;
    id main = reportedMain ?: (windows.count == 2 ? windows[1] : nil);
    if (focused && CFGetTypeID((__bridge CFTypeRef)focused) != AXUIElementGetTypeID()) focused = nil;
    if (main && CFGetTypeID((__bridge CFTypeRef)main) != AXUIElementGetTypeID()) main = nil;
    if (!focused) return main;
    if (main && !window_id(observation, (__bridge AXUIElementRef)focused) &&
        window_id(observation, (__bridge AXUIElementRef)main)) return main;
    return focused;
}

int64_t jot_focused_choice(void) {
    NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (!front || front.processIdentifier == getpid()) return 0;
    if (!AXIsProcessTrusted()) return -2;
    AXUIElementRef app = AXUIElementCreateApplication(front.processIdentifier);
    AXUIElementSetMessagingTimeout(app, 0.08f);
    id window = active_window(app, observed[@(front.processIdentifier)], nil);
    CFRelease(app);
    if (!window) return -2;
    CFTypeRef value = NULL;
    AXUIElementCopyAttributeValue((__bridge AXUIElementRef)window, kAXTitleAttribute, &value);
    id title = CFBridgingRelease(value);
    NSString *name = front.localizedName ?: @"当前应用";
    NSString *source = [title isKindOfClass:NSString.class] && [title length] ? [NSString stringWithFormat:@"%@ · %@", name, title] : name;
    return issue_drop_choice(@{@"application":front, @"window":window, @"source":source});
}

// 0 means the drag stayed in Jotiva, -1 is the desktop, -2 is an unidentified external window.
int64_t jot_source_at_drop(void) {
    bool switched = drag_in_progress && drag_target_pid != 0 && drag_target_pid != getpid() &&
        drag_target_pid == NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier;
    bool cancelled = drag_in_progress && drag_cancelled;
    jot_drag_ended();
    if (cancelled) return 0;
    if (switched) return jot_focused_choice();
    NSInteger hit = [NSWindow windowNumberAtPoint:NSEvent.mouseLocation belowWindowWithWindowNumber:0];
    NSArray *visible = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionIncludingWindow, (CGWindowID)hit));
    for (NSDictionary *item in visible) {
        NSInteger layer = [item[(__bridge NSString *)kCGWindowLayer] integerValue];
        if (layer < 0) return -1;
        pid_t pid = [item[(__bridge NSString *)kCGWindowOwnerPID] intValue];
        if (pid == getpid()) return 0;
        if (layer != 0 || !AXIsProcessTrusted()) return -2;
        NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
        NSNumber *number = item[(__bridge NSString *)kCGWindowNumber];
        for (NSDictionary *row in JotWindowIdentities(app, JotSystemWindows(kCGWindowListOptionAll))) {
            if ([row[@"cgId"] isEqual:number]) {
                return issue_drop_choice(row);
            }
        }
        return -2;
    }
    return -1;
}

static NSDictionary *observed_entry(uint64_t host) {
    for (NSNumber *pid in observed) {
        id handle = ((JotObservedApp *)observed[pid])->windows[@(host)];
        if (!handle) continue;
        NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:pid.intValue];
        if (app) return @{@"application": app, @"window": handle};
    }
    return nil;
}

static char *entry_identity(NSDictionary *entry) {
    if (!entry) return NULL;
    for (NSDictionary *row in JotWindowIdentities(entry[@"application"], JotSystemWindows(kCGWindowListOptionAll))) {
        if (![entry[@"window"] isEqual:row[@"window"]]) continue;
        NSData *data = [NSJSONSerialization dataWithJSONObject:row[@"identity"] options:0 error:nil];
        return strdup([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String);
    }
    return NULL;
}

char *jot_window_identity(uint64_t host) {
    return entry_identity(observed_entry(host));
}

void jot_capture_window_identity(uint64_t host, void (*done)(uint64_t, const char *)) {
    static NSMutableSet *inflight;
    if (!inflight) inflight = [NSMutableSet set];
    if ([inflight containsObject:@(host)]) return;
    NSDictionary *entry = observed_entry(host);
    if (!entry) return;
    [inflight addObject:@(host)];
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        char *json = entry_identity(entry);
        dispatch_async(dispatch_get_main_queue(), ^{
            [inflight removeObject:@(host)];
            if (json) { done(host, json); free(json); }
        });
    });
}

static void schedule_recovery(void) {
    if (!recovery_specs.count || !AXIsProcessTrusted()) return;
    if (recovery_running) { recovery_pending = true; return; }
    recovery_running = true;
    uint64_t epoch = recovery_epoch;
    NSDictionary *specs = recovery_specs;
    NSSet *apps = [NSSet setWithArray:[specs.allValues valueForKey:@"app"]];
    NSMutableArray *targets = [NSMutableArray array];
    for (NSRunningApplication *app in NSWorkspace.sharedWorkspace.runningApplications) {
        NSString *key = JotApplicationKey(app);
        if (key && [apps containsObject:key]) {
            [targets addObject:app];
            if (!observed[@(app.processIdentifier)]) {
                AXUIElementRef element = AXUIElementCreateApplication(app.processIdentifier);
                AXUIElementSetMessagingTimeout(element, 0.06f);
                observe_app(app, element);
                CFRelease(element);
            }
        }
    }
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        NSArray *system = targets.count ? JotSystemWindows(kCGWindowListOptionAll) : @[];
        NSMutableArray *rows = [NSMutableArray array];
        for (NSRunningApplication *app in targets) [rows addObjectsFromArray:JotWindowIdentities(app, system)];
        dispatch_async(dispatch_get_main_queue(), ^{
            if (epoch == recovery_epoch && !recovery_pending) {
                NSMutableDictionary *matches = JotRecoveryMatches(specs, rows, existing_windows, reopened_windows);
                for (NSString *origin in matches) {
                    NSDictionary *match = matches[origin];
                    NSRunningApplication *app = match[@"application"];
                    NSNumber *previous = window_id(observed[@(app.processIdentifier)], (__bridge AXUIElementRef)match[@"window"]);
                    JotHost target = register_entry(match);
                    NSData *data = [NSJSONSerialization dataWithJSONObject:match[@"identity"] options:0 error:nil];
                    if (target.id && !recover_host(origin.longLongValue, target.id,
                        [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String) && !previous)
                        jot_host_unwatch(target.id);
                }
            }
            recovery_running = false;
            if (recovery_pending) { recovery_pending = false; schedule_recovery(); }
        });
    });
}

void jot_set_recovery(const char *json, bool (*callback)(uint64_t, uint64_t, const char *)) {
    NSData *data = [NSData dataWithBytes:json length:strlen(json)];
    NSDictionary *next = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
    if ([next isEqual:recovery_specs]) return;
    if (!existing_windows) existing_windows = [NSMutableDictionary dictionary];
    for (NSString *key in existing_windows.allKeys) {
        if (![next[key] isEqual:recovery_specs[key]]) [existing_windows removeObjectForKey:key];
    }
    if (!next.count || ![[NSSet setWithArray:next.allKeys] isSubsetOfSet:[NSSet setWithArray:recovery_specs.allKeys ?: @[]]])
        reopened_windows = [NSMutableDictionary dictionary];
    recovery_specs = next;
    recover_host = callback;
    ++recovery_epoch;
    NSSet *apps = [NSSet setWithArray:[next.allValues valueForKey:@"app"]];
    recovery_apps = apps;
    for (NSNumber *pid in observed.allKeys) {
        NSRunningApplication *app = [NSRunningApplication runningApplicationWithProcessIdentifier:pid.intValue];
        NSString *key = JotApplicationKey(app);
        if (!((JotObservedApp *)observed[pid])->windows.count && !(key && [apps containsObject:key]))
            [observed removeObjectForKey:pid];
    }
    schedule_recovery();
}

static JotRect available_screen(CGPoint position, CGSize size) {
    NSArray<NSScreen *> *screens = NSScreen.screens;
    NSScreen *primary = screens.firstObject;
    if (!primary) return (JotRect){0};
    double top = NSMaxY(primary.frame);
    NSRect frame = NSMakeRect(position.x, top - position.y - MAX(1, size.height), MAX(1, size.width), MAX(1, size.height));
    NSScreen *chosen = primary;
    double largest = -1, nearest = INFINITY;
    for (NSScreen *screen in screens) {
        NSRect overlap = NSIntersectionRect(frame, screen.frame);
        double area = NSWidth(overlap) * NSHeight(overlap);
        double dx = MAX(0, MAX(NSMinX(screen.frame) - NSMaxX(frame), NSMinX(frame) - NSMaxX(screen.frame)));
        double dy = MAX(0, MAX(NSMinY(screen.frame) - NSMaxY(frame), NSMinY(frame) - NSMaxY(screen.frame)));
        double distance = dx * dx + dy * dy;
        if (area > largest || (area == largest && distance < nearest)) {
            chosen = screen;
            largest = area;
            nearest = distance;
        }
    }
    NSRect rect = chosen.visibleFrame;
    return (JotRect){rect.origin.x, top - NSMaxY(rect), rect.size.width, rect.size.height};
}

JotRect jot_screen_at(double x, double y) {
    return available_screen(CGPointMake(x, y), CGSizeZero);
}

CGPoint jot_window_origin(void *pointer) {
    NSRect frame = [(__bridge NSWindow *)pointer frame];
    return CGPointMake(NSMinX(frame), NSMaxY(NSScreen.screens.firstObject.frame) - NSMaxY(frame));
}

JotRect jot_visible_screen(void) {
    NSPoint point = NSEvent.mouseLocation;
    NSScreen *primary = NSScreen.screens.firstObject;
    return available_screen(CGPointMake(point.x, NSMaxY(primary.frame) - point.y), CGSizeZero);
}

CGPoint jot_pointer_position(void) {
    NSPoint point = NSEvent.mouseLocation;
    return CGPointMake(point.x, NSMaxY(NSScreen.screens.firstObject.frame) - point.y);
}

static JotRect window_screen(NSArray *attributes) {
    CGPoint origin;
    CGSize size;
    if (attributes.count >= 2 &&
        read_value((__bridge CFTypeRef)attributes[0], kAXValueCGPointType, &origin) &&
        read_value((__bridge CFTypeRef)attributes[1], kAXValueCGSizeType, &size)) {
        return available_screen(origin, size);
    }
    return jot_visible_screen();
}

JotHost jot_host_focused(bool attach) {
    JotHost result = {0};
    NSRunningApplication *front = NSWorkspace.sharedWorkspace.frontmostApplication;
    if (!front) return result;
    pid_t pid = front.processIdentifier;
    result.pid = pid;
    result.own_app = pid == NSProcessInfo.processInfo.processIdentifier;
    JotObservedApp *observation = observed[@(pid)];
    if (result.own_app || (!attach && !observation) || !AXIsProcessTrusted()) return result;

    AXUIElementRef application = observation ? (AXUIElementRef)CFRetain(observation->app) : AXUIElementCreateApplication(pid);
    // An application's first AX request may initialize its accessibility tree.
    // Keep the larger budget confined to this user-initiated, first lookup.
    if (!observation) AXUIElementSetMessagingTimeout(application, 0.4f);
    id reportedMain = attach ? nil : reported_main_windows[@(pid)];
    __attribute__((objc_precise_lifetime)) id handle = active_window(application, observation, reportedMain);
    if (!observation) AXUIElementSetMessagingTimeout(application, 0.05f);
    if (activating_pid == pid) {
        if (handle || reportedMain == NSNull.null) activating_pid = 0;
        else if (!attach) retry_activation(pid);
    }
    if (!handle) {
        CFRelease(application);
        return result;
    }
    result.has_window = true;
    AXUIElementRef window = (__bridge AXUIElementRef)handle;
    NSNumber *key = window_id(observation, window);
    if (key || attach) {
        bool needs_title = !key;
        NSArray *attributes = @[
            (__bridge NSString *)kAXPositionAttribute,
            (__bridge NSString *)kAXSizeAttribute,
            (__bridge NSString *)kAXMinimizedAttribute, @"AXFullScreen",
        ];
        if (needs_title) attributes = [attributes arrayByAddingObject:(__bridge NSString *)kAXTitleAttribute];
        CFArrayRef copied = NULL;
        AXUIElementCopyMultipleAttributeValues(window, (__bridge CFArrayRef)attributes, 0, &copied);
        NSArray *values = (__bridge_transfer NSArray *)copied;
        // Window identity comes from AX focus. A slow frame/title request must
        // not detach a known window or suppress its note until the next activation.
        if (!(values.count > 2 && (__bridge CFTypeRef)values[2] == kCFBooleanTrue)) {
            if (!key) key = watch_window(front, application, window);
            if (key) {
                result.id = key.unsignedLongLongValue;
                result.screen = window_screen(values);
                result.fullscreen = values.count > 3 && (__bridge CFTypeRef)values[3] == kCFBooleanTrue;
                if (needs_title) {
                    id title = values.count > 4 ? values[4] : nil;
                    NSString *suffix = [title isKindOfClass:NSString.class] ? title : @"";
                    NSString *app_name = front.localizedName ?: @"当前窗口";
                    NSString *source = suffix.length ? [NSString stringWithFormat:@"%@ · %@", app_name, suffix] : app_name;
                    strlcpy(result.source, source.UTF8String, sizeof(result.source));
                }
            }
        }
    }
    CFRelease(application);
    return result;
}

static bool activate(NSRunningApplication *application) {
    if (!application) return false;
    if (application.active) return true;
    [NSApp yieldActivationToApplication:application];
    return [application activateFromApplication:NSRunningApplication.currentApplication options:0];
}

JotHost jot_host_raise(uint64_t id) {
    JotHost target = {0};
    NSNumber *key = @(id);
    for (NSNumber *pid in observed) {
        JotObservedApp *observation = observed[pid];
        AXUIElementRef window = (__bridge AXUIElementRef)observation->windows[key];
        if (!window) continue;
        NSRunningApplication *application = [NSRunningApplication runningApplicationWithProcessIdentifier:pid.intValue];
        if (!activate(application)) break;
        AXUIElementSetAttributeValue(window, kAXMinimizedAttribute, kCFBooleanFalse);
        AXError main = AXUIElementSetAttributeValue(window, kAXMainAttribute, kCFBooleanTrue);
        bool raised = AXUIElementPerformAction(window, kAXRaiseAction) == kAXErrorSuccess;
        if (!raised && main != kAXErrorSuccess) break;
        target.id = id;
        target.pid = pid.intValue;
        CFArrayRef values = NULL;
        NSArray *attributes = @[(__bridge NSString *)kAXPositionAttribute, (__bridge NSString *)kAXSizeAttribute];
        AXUIElementCopyMultipleAttributeValues(window, (__bridge CFArrayRef)attributes, 0, &values);
        target.screen = window_screen(CFBridgingRelease(values));
        break;
    }
    return target;
}

void jot_host_activate(pid_t pid) {
    activate([NSRunningApplication runningApplicationWithProcessIdentifier:pid]);
}

void jot_host_unwatch(uint64_t id) {
    NSNumber *key = @(id);
    for (NSNumber *pid in observed.allKeys) {
        JotObservedApp *observation = observed[pid];
        AXUIElementRef window = (__bridge AXUIElementRef)observation->windows[key];
        if (!window) continue;
        CFStringRef events[] = {
            kAXUIElementDestroyedNotification,
            kAXWindowMiniaturizedNotification, kAXWindowDeminiaturizedNotification, kAXMovedNotification, kAXResizedNotification
        };
        for (size_t i = 0; i < sizeof(events) / sizeof(events[0]); ++i) {
            AXObserverRemoveNotification(observation->observer, window, events[i]);
        }
        CFDictionaryRemoveValue(observation->identities, window);
        [observation->windows removeObjectForKey:key];
        if (!observation->windows.count) [observed removeObjectForKey:pid];
        return;
    }
}

void jot_show_passive(void *window) {
    [(__bridge NSWindow *)window orderFrontRegardless];
}
