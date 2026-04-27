pub const CRATE_NAME: &str = "workbench-api";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "workbench-api");
    }
}
