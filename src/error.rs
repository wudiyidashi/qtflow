use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum QtflowError {
    #[error("{0}")]
    ConfigOrArg(String),

    #[error("required tool not found: {0}")]
    ToolNotFound(String),

    #[error("project root not found from {start}: no ancestor contains CMakeLists.txt")]
    ProjectRootNotFound { start: PathBuf },

    #[error("config file not found: {0}")]
    ConfigNotFound(PathBuf),

    #[error("failed to read config file {path}: {source}")]
    ConfigRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse config file {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },

    #[error("environment bootstrap failed: {0}")]
    EnvironmentBootstrap(String),

    #[error("diagnostic fatal setup issue: {0}")]
    DiagnosticFatal(String),

    #[error("command failed: {0}")]
    CommandFailed(String),

    #[error("reported failure")]
    ReportedFailure { exit_code: i32 },

    #[error("failed to spawn command '{program}': {source}")]
    CommandSpawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
}

impl QtflowError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::CommandFailed(_) => 1,
            Self::ReportedFailure { exit_code } => *exit_code,
            Self::ConfigOrArg(_) | Self::ConfigParse { .. } => 2,
            Self::ToolNotFound(_) | Self::CommandSpawn { .. } => 3,
            Self::ProjectRootNotFound { .. }
            | Self::ConfigNotFound(_)
            | Self::ConfigRead { .. } => 4,
            Self::EnvironmentBootstrap(_) => 5,
            Self::DiagnosticFatal(_) => 6,
        }
    }

    pub fn not_yet_implemented(command: impl Into<String>) -> Self {
        Self::ConfigOrArg(format!(
            "{} is not yet implemented in this milestone",
            command.into()
        ))
    }

    pub fn already_reported(&self) -> bool {
        matches!(self, Self::ReportedFailure { .. })
    }
}
