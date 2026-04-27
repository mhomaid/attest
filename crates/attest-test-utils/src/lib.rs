pub const CRATE_NAME: &str = "attest-test-utils";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "attest-test-utils");
    }
}
