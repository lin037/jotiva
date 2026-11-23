// No windows or Accessibility permission required: exercise the snapshot held around AX enumeration.
#import "../../src-tauri/src/host/macos.m"

static uint64_t publish(NSString *owner, uint64_t generation, NSDictionary *window) {
    NSMutableArray *rows = [@[@{@"app": @"会议", @"title": @"会议室"}] mutableCopy];
    return publish_choices(owner, generation, rows, @[window]) ? [rows[0][@"choice"] unsignedLongLongValue] : 0;
}

int main(void) {
    @autoreleasepool {
        NSDictionary *first = @{@"marker": @"A"};
        NSDictionary *second = @{@"marker": @"B"};
        uint64_t main = jot_begin_source_choices("main");
        uint64_t quick = jot_begin_source_choices("quick");
        uint64_t mainChoice = publish(@"main", main, first);
        uint64_t quickChoice = publish(@"quick", quick, second);
        assert(mainChoice && quickChoice && mainChoice != quickChoice);
        assert(entry_for_choice(mainChoice) == first && entry_for_choice(quickChoice) == second);

        // Another scan of the same picker supersedes its handles, including a late completion.
        uint64_t refreshed = jot_begin_source_choices("main");
        assert(!publish(@"main", main, first));
        assert(!entry_for_choice(mainChoice) && entry_for_choice(quickChoice) == second);
        mainChoice = publish(@"main", refreshed, first);
        assert(mainChoice);
        uint64_t drop = issue_drop_choice(second);
        jot_clear_source_choices("quick");
        assert(!entry_for_choice(quickChoice) && !publish(@"quick", quick, second));
        assert(entry_for_choice(mainChoice) == first && entry_for_choice(drop) == second);
        jot_clear_source_choices("main");
        assert(source_snapshots.count == 0 && !entry_for_choice(mainChoice));
        puts("Two pickers retained independent choices. Refresh, late replies, close and drag selection passed.");
    }
}
