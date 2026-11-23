// Exercise the production credential format with an in-memory Keychain.
#import <Foundation/Foundation.h>
#import <Security/Security.h>
#import <LocalAuthentication/LocalAuthentication.h>
#include <assert.h>

static NSMutableDictionary *items;
static BOOL locked;
static NSDictionary *dictionary(CFDictionaryRef value) { return (__bridge NSDictionary *)value; }
static NSArray *identity(NSDictionary *query) {
    return @[query[(__bridge id)kSecAttrService], query[(__bridge id)kSecAttrAccount]];
}
static OSStatus fixtureLookup(CFDictionaryRef query, CFTypeRef *result) {
    NSDictionary *attrs = dictionary(query);
    LAContext *context = attrs[(__bridge id)kSecUseAuthenticationContext];
    if (context) assert(context.interactionNotAllowed);
    NSDictionary *item = items[identity(attrs)];
    if (!item) return errSecItemNotFound;
    if (locked) return errSecInteractionNotAllowed;
    if (result) *result = CFBridgingRetain(item[(__bridge id)kSecValueData]);
    return errSecSuccess;
}
static OSStatus fixtureAdd(CFDictionaryRef query, CFTypeRef *result) {
    NSDictionary *attrs = dictionary(query);
    NSArray *key = identity(attrs);
    if (items[key]) return errSecDuplicateItem;
    items[key] = attrs.mutableCopy;
    return errSecSuccess;
}
static OSStatus fixtureUpdate(CFDictionaryRef query, CFDictionaryRef attributes) {
    NSArray *key = identity(dictionary(query));
    NSMutableDictionary *item = items[key];
    if (!item) return errSecItemNotFound;
    [items removeObjectForKey:key];
    [item addEntriesFromDictionary:dictionary(attributes)];
    items[identity(item)] = item;
    return errSecSuccess;
}
static OSStatus fixtureErase(CFDictionaryRef query) {
    NSArray *key = identity(dictionary(query));
    if (!items[key]) return errSecItemNotFound;
    [items removeObjectForKey:key];
    return errSecSuccess;
}

#define SecItemCopyMatching fixtureLookup
#define SecItemAdd fixtureAdd
#define SecItemUpdate fixtureUpdate
#define SecItemDelete fixtureErase
#import "../../src-tauri/src/settings/filing.m"

int main(void) {
    @autoreleasepool {
        items = [NSMutableDictionary new];
        const char *service = "test.jot.example";
        const char *original = "https://one.invalid/classify";
        const char *other = "https://two.invalid/classify";
        assert(jot_filing_credential(service, original, 0) == 0);
        assert(jot_filing_key(service, original, "bad\r\nvalue") == -3);
        assert(jot_filing_credential(service, original, 0) == 0);
        assert(jot_filing_key(service, original, "fixture-only") == 1);
        assert(jot_filing_key(service, original, NULL) == 1);
        assert(jot_filing_key("different.app", original, NULL) == 0);
        char *reply = NULL;
        assert(allowedTransport("http://127.0.0.1:8000/gateway/v1/systemone"));
        assert(allowedTransport("http://[::1]:8000/v1/systemone"));
        assert(!allowedTransport("http://localhost:8000/v1/systemone"));
        assert(!allowedTransport("http://127.0.0.1:8000@host.invalid/classify"));
        assert(jot_filing_post(service, other, "{}", &reply) == -1 && !reply);
        assert(jot_filing_key(service, "https://one.invalid/other-path", NULL) == 0);
        assert(jot_filing_key(service, other, "replacement-fixture") == 1);
        assert(jot_filing_key(service, original, NULL) == 1);
        assert(jot_filing_key(service, other, NULL) == 1);
        locked = YES;
        assert(jot_filing_credential(service, original, 0) == -1);
        locked = NO;
        assert(jot_filing_post(service, "http://two.invalid/classify", "{}", &reply) == -4 && !reply);
        assert(jot_filing_credential(service, original, 2) == 0);
        assert(jot_filing_credential(service, other, 0) == 1);

        // Earlier builds kept one endpoint-bound record under the account "Jev".
        NSMutableDictionary *legacy = [keyQuery(service, nil) mutableCopy];
        NSDictionary *record = @{@"endpoint":@(original), @"key":@"legacy-fixture"};
        legacy[(__bridge id)kSecValueData] = [NSJSONSerialization dataWithJSONObject:record options:0 error:NULL];
        items[identity(legacy)] = legacy;
        assert(jot_filing_key(service, original, NULL) == 1);
        assert(jot_filing_credential(service, other, 2) == 0);
        assert(jot_filing_key(service, other, NULL) == 0);
        assert(jot_filing_key(service, original, NULL) == 1);
        assert(jot_filing_key(service, original, "new-fixture") == 1);
        assert(!items[identity(keyQuery(service, nil))]);
        NSString *resolved = nil;
        assert(lookupKey(service, @(original), NO, &resolved) == errSecSuccess);
        assert([resolved isEqual:@"new-fixture"]);
        assert(jot_filing_credential(service, original, 2) == 0);
        assert(jot_filing_key(service, original, NULL) == 0);
        assert(!items.count);

        // The oldest format has no endpoint. Never use it for a network request.
        legacy = [keyQuery(service, nil) mutableCopy];
        legacy[(__bridge id)kSecValueData] = [@"legacy-fixture" dataUsingEncoding:NSUTF8StringEncoding];
        items[identity(legacy)] = legacy;
        assert(jot_filing_post(service, original, "{}", &reply) == -1 && !reply);
    }
}
