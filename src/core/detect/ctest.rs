use super::Probe;

pub fn version(probe: &impl Probe, program: &str) -> Option<String> {
    probe.version(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::detect::test_support::FakeProbe;

    #[test]
    fn version_uses_injected_probe() {
        let probe = FakeProbe::new([("custom-ctest", Some("ctest version 3.30.0".to_string()))]);

        assert_eq!(
            version(&probe, "custom-ctest").as_deref(),
            Some("ctest version 3.30.0")
        );
    }
}
