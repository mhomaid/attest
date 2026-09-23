//! Shared helpers for the live end-to-end suites under `tests/`.

/// Live suites need a running stack (`make dev-up`), so they only run with `ATTEST_E2E=1`.
pub fn e2e_enabled() -> bool {
    std::env::var("ATTEST_E2E").as_deref() == Ok("1")
}

/// Return early from a test unless live end-to-end tests are enabled.
#[macro_export]
macro_rules! require_e2e {
    () => {
        if !$crate::e2e_enabled() {
            eprintln!("SKIP: set ATTEST_E2E=1 with a running stack");
            return;
        }
    };
}
