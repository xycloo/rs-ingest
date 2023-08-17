/// Context path object.
pub struct ContextPath(pub String);

impl Default for ContextPath {
    fn default() -> Self {
        Self("/tmp/rs_ingestion_temp".to_string())
    }
}

/// Enum describing the network to run stellar-core on.
#[derive(Copy, Clone)]
pub enum SupportedNetwork {
    /// Ingest on futurenet.
    Futurenet,

    /// Ingest on pubnet.
    Pubnet
}

/// Configuration settings
pub struct IngestionConfig {

    /// Path to the stellar-core executable.
    pub executable_path: String,

    /// Path to the context directory.
    /// The context directory is where temporary buckets
    /// database, and toml configuration are stored.
    pub context_path: ContextPath,

    /// Network to run stellar-core on.
    pub network: SupportedNetwork
}