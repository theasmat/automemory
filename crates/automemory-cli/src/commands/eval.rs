use anyhow::Result;

/// Runs the local deterministic eval suites (retrieval precision, stale-flag
/// precision, contradiction detection, lease races, hook latency).
/// Exit code is the judge: 0 when every suite passes, 1 otherwise.
pub fn run(json: bool) -> Result<()> {
    let report = automemory_core::eval::run_local_suites();
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", report.summary());
    }
    if !report.passed() {
        std::process::exit(1);
    }
    Ok(())
}
