#import <AppKit/AppKit.h>
#import <Security/Security.h>
#import <LocalAuthentication/LocalAuthentication.h>
#import <curl/curl.h>

static NSString *credentialName(const char *service) {
    return [[NSString stringWithUTF8String:service] hasSuffix:@".images"] ? @"Images" : @"Jev";
}

static NSDictionary *keyQuery(const char *service, NSString *endpoint) {
    NSString *name = credentialName(service);
    return @{ (__bridge id)kSecClass: (__bridge id)kSecClassGenericPassword,
              (__bridge id)kSecAttrService: [NSString stringWithUTF8String:service],
              (__bridge id)kSecAttrAccount: endpoint ? [NSString stringWithFormat:@"%@ %@", name, endpoint] : name };
}

static BOOL validKey(NSString *key) {
    if (!key.length) return NO;
    for (NSUInteger i = 0; i < key.length; ++i) {
        unichar c = [key characterAtIndex:i];
        if (c < 0x21 || c > 0x7e) return NO;
    }
    return YES;
}

// A key belongs to the exact URL it was supplied for. Old unscoped credentials
// cannot authenticate to a new address. The WebView sees only their presence.
static OSStatus readKey(NSDictionary *query, NSString *endpoint, BOOL interactive, NSString **key) {
    NSMutableDictionary *lookup = query.mutableCopy;
    lookup[(__bridge id)kSecReturnData] = @YES;
    if (!interactive) {
        LAContext *context = [LAContext new];
        context.interactionNotAllowed = YES;
        lookup[(__bridge id)kSecUseAuthenticationContext] = context;
    }
    CFTypeRef found = NULL;
    OSStatus code = SecItemCopyMatching((__bridge CFDictionaryRef)lookup, &found);
    if (code != errSecSuccess) return code;
    NSData *data = CFBridgingRelease(found);
    if (![data isKindOfClass:NSData.class]) return errSecItemNotFound;
    id record = [NSJSONSerialization JSONObjectWithData:data options:0 error:NULL];
    if (![record isKindOfClass:NSDictionary.class] || ![record[@"endpoint"] isEqual:endpoint]
        || ![record[@"key"] isKindOfClass:NSString.class] || !validKey(record[@"key"])) return errSecItemNotFound;
    *key = record[@"key"];
    return errSecSuccess;
}

static OSStatus lookupKey(const char *service, NSString *endpoint, BOOL interactive, NSString **key) {
    OSStatus code = readKey(keyQuery(service, endpoint), endpoint, interactive, key);
    if (code == errSecItemNotFound) {
        // Earlier builds stored a single endpoint-bound JSON record under the account "Jev".
        code = readKey(keyQuery(service, nil), endpoint, interactive, key);
    }
    return code;
}

static int jot_filing_key(const char *service, const char *endpoint, const char *key) {
    @autoreleasepool {
        NSString *url = [NSString stringWithUTF8String:endpoint];
        NSDictionary *query = keyQuery(service, url);
        if (!key) {
            NSString *existing = nil;
            OSStatus code = lookupKey(service, url, NO, &existing);
            return code == errSecSuccess ? 1 : code == errSecItemNotFound ? 0 : -1;
        }
        if (!*key) {
            NSDictionary *legacy = keyQuery(service, nil);
            NSString *old = nil;
            OSStatus code = readKey(legacy, url, YES, &old);
            if (code == errSecSuccess) code = SecItemDelete((__bridge CFDictionaryRef)legacy);
            if (code != errSecSuccess && code != errSecItemNotFound) return -1;
            code = SecItemDelete((__bridge CFDictionaryRef)query);
            return code == errSecSuccess || code == errSecItemNotFound ? 0 : -1;
        }
        NSString *secret = [NSString stringWithUTF8String:key];
        if (!validKey(secret)) return -3;
        NSDictionary *record = @{ @"endpoint": url, @"key": secret };
        NSData *data = [NSJSONSerialization dataWithJSONObject:record options:0 error:NULL];
        if (!data) return -1;
        NSDictionary *update = @{ (__bridge id)kSecValueData: data,
                                  (__bridge id)kSecAttrLabel: [@"Jotiva · " stringByAppendingString:credentialName(service)] };
        OSStatus code = SecItemUpdate((__bridge CFDictionaryRef)query, (__bridge CFDictionaryRef)update);
        if (code == errSecItemNotFound) {
            NSDictionary *legacy = keyQuery(service, nil);
            NSString *old = nil;
            code = readKey(legacy, url, YES, &old);
            if (code == errSecSuccess) {
                // Rotate a legacy key in place so its previous value is not retained.
                NSMutableDictionary *move = update.mutableCopy;
                move[(__bridge id)kSecAttrAccount] = query[(__bridge id)kSecAttrAccount];
                code = SecItemUpdate((__bridge CFDictionaryRef)legacy, (__bridge CFDictionaryRef)move);
            } else if (code == errSecItemNotFound) {
                NSMutableDictionary *item = query.mutableCopy;
                [item addEntriesFromDictionary:update];
                code = SecItemAdd((__bridge CFDictionaryRef)item, NULL);
            }
        }
        return code == errSecSuccess ? 1 : -1;
    }
}

// Only presence, cancellation or success crosses the settings IPC boundary.
int jot_filing_credential(const char *service, const char *endpoint, int action) {
    @autoreleasepool {
        if (action == 0) return jot_filing_key(service, endpoint, NULL);
        if (action == 2) return jot_filing_key(service, endpoint, "");
        if (action != 1) return -1;
        __block NSString *secret = nil;
        void (^prompt)(void) = ^{
            NSAlert *alert = [NSAlert new];
            alert.messageText = [credentialName(service) isEqualToString:@"Images"] ? @"保存图片上传密钥" : @"保存 Jev API 密钥";
            alert.informativeText = [NSString stringWithFormat:@"此密钥将用于以下地址，并保存在 macOS 钥匙串中。\n\n%@", [NSString stringWithUTF8String:endpoint]];
            NSSecureTextField *field = [[NSSecureTextField alloc] initWithFrame:NSMakeRect(0, 0, 340, 24)];
            field.placeholderString = @"API 密钥";
            [field setAccessibilityLabel:@"API 密钥"];
            alert.accessoryView = field;
            [alert addButtonWithTitle:@"存入钥匙串"];
            [alert addButtonWithTitle:@"取消"].keyEquivalent = @"\e";
            alert.window.initialFirstResponder = field;
            if ([alert runModal] == NSAlertFirstButtonReturn) secret = field.stringValue;
            field.stringValue = @"";
        };
        if ([NSThread isMainThread]) prompt();
        else dispatch_sync(dispatch_get_main_queue(), prompt);
        if (!secret) return -2;
        if (!validKey(secret)) return -3;
        return jot_filing_key(service, endpoint, secret.UTF8String);
    }
}

static size_t receiveBody(char *bytes, size_t size, size_t count, void *context) {
    NSMutableData *data = (__bridge NSMutableData *)context;
    if (size && count > (128 * 1024 - data.length) / size) return 0;
    [data appendBytes:bytes length:size * count];
    return size * count;
}

// Rust supplies a canonical URL. Check the literal authority again before using a credential.
static BOOL loopbackAddress(const char *url) {
    const char *host = strstr(url, "://");
    if (!host) return NO;
    host += 3;
    const char *end = host + strcspn(host, "/?#");
    const char *port;
    if (strncmp(host, "[::1]", 5) == 0) port = host + 5;
    else if (strncmp(host, "127.0.0.1", 9) == 0) port = host + 9;
    else return NO;
    if (port == end) return YES;
    if (port >= end || *port++ != ':' || port == end) return NO;
    return strspn(port, "0123456789") == (size_t)(end - port);
}

static BOOL allowedTransport(const char *url) {
    return strncmp(url, "https://", 8) == 0 || (strncmp(url, "http://", 7) == 0 && loopbackAddress(url));
}

long jot_filing_http(const char *url, const char *key, const char *payload, long timeout, char **reply) {
    @autoreleasepool {
        if (!allowedTransport(url)) return -4;
        static dispatch_once_t once;
        static CURLcode initialized;
        dispatch_once(&once, ^{ initialized = curl_global_init(CURL_GLOBAL_DEFAULT); });
        if (initialized != CURLE_OK) return -4;
        CURL *curl = curl_easy_init();
        if (!curl) return -4;
        NSString *token = [NSString stringWithUTF8String:key];
        if (!validKey(token)) { curl_easy_cleanup(curl); return -3; }
        struct curl_slist *headers = curl_slist_append(NULL, "Content-Type: application/json");
        NSString *authorization = [@"Authorization: Bearer " stringByAppendingString:token];
        struct curl_slist *authorized = curl_slist_append(headers, authorization.UTF8String);
        NSMutableData *body = [NSMutableData data];
        long status = -4;
        if (headers && authorized) {
            curl_easy_setopt(curl, CURLOPT_URL, url);
            curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 0L);
            curl_easy_setopt(curl, CURLOPT_SSL_VERIFYPEER, 1L);
            curl_easy_setopt(curl, CURLOPT_SSL_VERIFYHOST, 2L);
            curl_easy_setopt(curl, CURLOPT_HEADEROPT, (long)CURLHEADER_SEPARATE);
            if (loopbackAddress(url)) curl_easy_setopt(curl, CURLOPT_PROXY, "");
            curl_easy_setopt(curl, CURLOPT_HTTPHEADER, authorized);
            curl_easy_setopt(curl, CURLOPT_POSTFIELDS, payload);
            curl_easy_setopt(curl, CURLOPT_TIMEOUT, timeout);
            curl_easy_setopt(curl, CURLOPT_NOSIGNAL, 1L);
            curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, receiveBody);
            curl_easy_setopt(curl, CURLOPT_WRITEDATA, (__bridge void *)body);
            CURLcode code = curl_easy_perform(curl);
            switch (code) {
                case CURLE_OK:
                    curl_easy_getinfo(curl, CURLINFO_RESPONSE_CODE, &status);
                    if (status == 200) {
                        if (body.length && memchr(body.bytes, 0, body.length)) { status = -3; break; }
                        [body appendBytes:"" length:1];
                        *reply = strdup(body.bytes);
                        if (!*reply) status = -3;
                    }
                    break;
                case CURLE_OPERATION_TIMEDOUT: status = -2; break;
                case CURLE_COULDNT_CONNECT: case CURLE_COULDNT_RESOLVE_HOST:
                case CURLE_SEND_ERROR: case CURLE_RECV_ERROR: status = 0; break;
                case CURLE_WRITE_ERROR: status = -3; break;
                default: break;
            }
        }
        curl_slist_free_all(authorized ?: headers);
        curl_easy_cleanup(curl);
        return status;
    }
}

long jot_filing_post(const char *service, const char *endpoint, const char *payload, char **reply) {
    @autoreleasepool {
        if (!allowedTransport(endpoint)) return -4;
        NSString *key = nil;
        OSStatus status = lookupKey(service, [NSString stringWithUTF8String:endpoint], YES, &key);
        if (status != errSecSuccess || !key.length) return -1;
        return jot_filing_http(endpoint, key.UTF8String, payload, 25L, reply);
    }
}

int jot_read_endpoint_key(const char *service, const char *endpoint, char **out) {
    @autoreleasepool {
        NSString *value = nil;
        OSStatus status = lookupKey(service, [NSString stringWithUTF8String:endpoint], YES, &value);
        if (status == errSecItemNotFound) return 0;
        if (status != errSecSuccess) return -1;
        *out = strdup(value.UTF8String);
        return *out ? 1 : -1;
    }
}
