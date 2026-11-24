#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>

NSString *JotApplicationKey(NSRunningApplication *app);
NSArray<NSDictionary *> *JotWindowIdentities(NSRunningApplication *app, NSArray *cgWindows);
NSArray *JotSystemWindows(CGWindowListOption options);

NSMutableDictionary *JotRecoveryMatches(NSDictionary *specs, NSArray *rows, NSMutableDictionary *initialWindows,
    NSMutableDictionary *createdWindows);
