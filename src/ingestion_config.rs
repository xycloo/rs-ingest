/// Context path object.
pub struct ContextPath(pub String);

impl Default for ContextPath {
    fn default() -> Self {
        Self("/tmp/rs_ingestion_temp".to_string())
    }
}

/// Enum describing the network to run stellar-core on.
#[derive(Copy, Clone, Debug)]
pub enum SupportedNetwork {
    /// Ingest on futurenet.
    Futurenet,

    /// Ingest on pubnet.
    Pubnet,

    /// Ingest on testnet.
    Testnet
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
    pub network: SupportedNetwork,

    /// Option to create bounded buffer size.
    /// By default, rs-ingest will use unbounded
    /// buffers, but in some cases the implementor
    /// might want to specify a buffer size to
    /// adapt to how they handle the receiver.
    pub bounded_buffer_size: Option<usize>,

    /// Option to split multi-thread mode catchups
    /// to produce staggered and help with write
    /// amount in databases for large catchups.
    /// 
    /// This option will help to stagger large catchup
    /// data, enabling for checkpoints.
    /// 
    /// This option is not a good approach in most
    /// cases as it will slow down the catchup process, 
    /// make sure you understand what it does
    /// and try out bounded buffers or
    /// handling large catchup data yourself first.
    pub staggered: Option<u32>,
}
