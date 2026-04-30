#![doc = include_str!(concat!(env!("OUT_DIR"), "/docs.md"))]

#[cfg(test)]
mod tests {
    #[test]
    fn validate_templates() {
        // INVERTED: templates contain Cargo.toml which cargo excludes from
        // the tarball. Flip once templates use _Cargo.toml.
        ::battery_pack::testing::validate_templates(env!("CARGO_MANIFEST_DIR")).unwrap_err();
    }
}
