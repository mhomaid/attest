pub const CRATE_NAME: &str = "attest-detection-dsl";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "attest-detection-dsl");
    }
}
