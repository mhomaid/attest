pub const CRATE_NAME: &str = "attest-storage-clickhouse";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "attest-storage-clickhouse");
    }
}
