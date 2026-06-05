use std::process::Command;

pub mod builddir;
pub mod cmake;
pub mod ctest;
pub mod msvc;
pub mod qt;

pub trait Probe {
    fn version(&self, program: &str) -> Option<String>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemProbe;

impl Probe for SystemProbe {
    fn version(&self, program: &str) -> Option<String> {
        let output = Command::new(program).arg("--version").output().ok()?;
        if !output.status.success() {
            return None;
        }

        let mut text = String::from_utf8_lossy(&output.stdout).to_string();
        if text.trim().is_empty() {
            text = String::from_utf8_lossy(&output.stderr).to_string();
        }

        first_version_line(&text)
    }
}

fn first_version_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
pub mod test_support {
    use std::collections::BTreeMap;

    use super::Probe;

    #[derive(Debug, Clone, Default)]
    pub struct FakeProbe {
        versions: BTreeMap<String, Option<String>>,
    }

    impl FakeProbe {
        pub fn new(
            versions: impl IntoIterator<Item = (impl Into<String>, Option<String>)>,
        ) -> Self {
            Self {
                versions: versions
                    .into_iter()
                    .map(|(program, version)| (program.into(), version))
                    .collect(),
            }
        }
    }

    impl Probe for FakeProbe {
        fn version(&self, program: &str) -> Option<String> {
            self.versions.get(program).cloned().flatten()
        }
    }
}
