pub const CRATE_NAME: &str = "attest-orchestrator";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "attest-orchestrator");
    }
}
