use super::WatchOptions;

#[test]
fn interactive_watch_is_bounded() {
    let options = WatchOptions::interactive();

    assert_eq!(options.interval_ms, 500);
    assert_eq!(options.max_ticks, 12);
    assert!(!options.keep_waiting_for_approval);
}
