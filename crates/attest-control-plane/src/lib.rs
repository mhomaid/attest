pub const CRATE_NAME: &str = "attest-control-plane";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "attest-control-plane");
    }
}
