use trainer::recorder::estimate_polling_hz;

#[test]
fn test_estimate_polling_hz_1000hz_with_jitter() {
    // 1000 Hz = 1000 us interval. With small jitter: 980, 1020, 1000...
    let mut timestamps = Vec::new();
    let mut t = 1_000_000u64;
    timestamps.push(t);
    for i in 0..100 {
        let delta = if i % 2 == 0 { 990 } else { 1010 };
        t += delta;
        timestamps.push(t);
    }
    assert_eq!(estimate_polling_hz(&timestamps), 1000);
}

#[test]
fn test_estimate_polling_hz_500hz() {
    // 500 Hz = 2000 us interval
    let mut timestamps = Vec::new();
    let mut t = 1_000_000u64;
    timestamps.push(t);
    for _ in 0..100 {
        t += 2000;
        timestamps.push(t);
    }
    assert_eq!(estimate_polling_hz(&timestamps), 500);
}

#[test]
fn test_estimate_polling_hz_8000hz() {
    // 8000 Hz = 125 us interval
    let mut timestamps = Vec::new();
    let mut t = 1_000_000u64;
    timestamps.push(t);
    for _ in 0..200 {
        t += 125;
        timestamps.push(t);
    }
    assert_eq!(estimate_polling_hz(&timestamps), 8000);
}

#[test]
fn test_estimate_polling_hz_bursts_with_idle_gaps() {
    // Bursts of 1000 Hz (1000 us) with idle gaps of 100_000 us (> 50 ms)
    let mut timestamps = Vec::new();
    let mut t = 1_000_000u64;
    timestamps.push(t);
    for _ in 0..5 {
        for _ in 0..20 {
            t += 1000;
            timestamps.push(t);
        }
        // Idle gap: 100 ms > 50 ms
        t += 100_000;
        timestamps.push(t);
    }
    assert_eq!(estimate_polling_hz(&timestamps), 1000);
}

#[test]
fn test_estimate_polling_hz_too_few_events() {
    // Fewer than required events -> fallback 1000 Hz
    let timestamps_empty: Vec<u64> = vec![];
    assert_eq!(estimate_polling_hz(&timestamps_empty), 1000);

    let timestamps_few = vec![1_000_000, 1_002_000];
    assert_eq!(estimate_polling_hz(&timestamps_few), 1000);
}

#[test]
fn test_estimate_polling_hz_non_standard_rate() {
    // Non-standard interval: e.g. 1500 us (666.67 Hz) -> not within 10% of 500 or 1000 -> rounds to 667
    let mut timestamps = Vec::new();
    let mut t = 1_000_000u64;
    timestamps.push(t);
    for _ in 0..50 {
        t += 1500;
        timestamps.push(t);
    }
    assert_eq!(estimate_polling_hz(&timestamps), 667);
}
