#import "../../src-tauri/src/platform/macos/objc/filing.m"

int main(int argc, const char *argv[]) {
    if (argc != 3 || !loopbackAddress(argv[1])) return 2;
    char *reply = NULL;
    long status = jot_filing_http(argv[1], "qa-fixture-key", argv[2], 1L, &reply);
    printf("%ld\n", status);
    if (reply) {
        fputs(reply, stdout);
        free(reply);
    }
    return 0;
}
