use crate::{
    toml::generate_predefined_cfg, BufferedLedgerMetaReaderMode, IngestionConfig, MetaResult,
    RunnerError, StellarCoreRunner, StellarCoreRunnerPublic,
};
use std::sync::mpsc::Receiver;
use stellar_xdr::next::LedgerCloseMeta;

#[derive(Clone, Copy)]
/// Represents a bounded range
pub struct BoundedRange(pub u32, pub u32);

/// Ranges supported.
/// Currently unbounded ranges are not supported.
pub enum Range {
    /// Bounded range
    Bounded(BoundedRange),
}

impl From<Range> for std::ops::Range<u32> {
    fn from(range: Range) -> Self {
        match range {
            Range::Bounded(bounded_range) => bounded_range.0..bounded_range.1,
        }
    }
}

impl Range {
    /// Gets a tuple representation of the range
    pub fn bounded(&self) -> (u32, u32) {
        match self {
            Range::Bounded(bounded_range) => (bounded_range.0, bounded_range.1),
        }
    }
}

/// Enum to represent different types of errors related to the CaptiveCore.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Error encountered while running the core using StellarCoreRunner.
    #[error("Error while running core: {0}")]
    Core(#[from] RunnerError),

    /// The requested ledger was not found in the prepared ledgers.
    #[error("Requested ledger was not found in prepared ledgers")]
    LedgerNotFound,

    /// An attempt was made to call the closing mechanism, but the core is running in single-thread mode.
    #[error("Called closing mechanism, but core is running in single-thread mode")]
    CloseOnSingleThread,
}

/// Represents a captive instance of the Stellar Core.
pub struct CaptiveCore {
    /// The Stellar Core runner associated with the captive instance.
    pub stellar_core_runner: StellarCoreRunner,
}

impl CaptiveCore {
    /// Creates a new CaptiveCore instance
    pub fn new(config: IngestionConfig) -> Self {
        // generate configs in path
        generate_predefined_cfg(&config.context_path.0, config.network);

        Self {
            stellar_core_runner: StellarCoreRunner::new(config),
        }
    }

    fn offline_replay_single_thread(&mut self, from: u32, to: u32) -> Result<(), Error> {
        // TODO: get archiver last checkpoint ledger for error accuracy.

        self.stellar_core_runner.catchup_single_thread(from, to)?;

        Ok(())
    }

    fn offline_replay_multi_thread(
        &mut self,
        from: u32,
        to: u32,
    ) -> Result<Receiver<Box<MetaResult>>, Error> {
        Ok(self.stellar_core_runner.catchup_multi_thread(from, to)?)
    }

    /// Prepares ledgers in single-thread mode based on the specified range.
    ///
    /// # Arguments
    ///
    /// * `range` - The range of ledgers to prepare.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if preparation is successful, or an `Error` if an issue occurs.
    pub fn prepare_ledgers_single_thread(&mut self, range: &Range) -> Result<(), Error> {
        match range {
            Range::Bounded(range) => {
                self.offline_replay_single_thread(range.0, range.1)?;
            }
        };

        Ok(())
    }

    /// Prepares ledgers in multi-thread mode based on the specified range.
    ///
    /// # Arguments
    ///
    /// * `range` - The range of ledgers to prepare.
    ///
    /// # Returns
    ///
    /// Returns a channel receiver for receiving metadata results if preparation is successful,
    /// or an `Error` if an issue occurs.
    pub fn prepare_ledgers_multi_thread(
        &mut self,
        range: &Range,
    ) -> Result<Receiver<Box<MetaResult>>, Error> {
        let receiver = match range {
            Range::Bounded(range) => self.offline_replay_multi_thread(range.0, range.1)?,
        };

        Ok(receiver)
    }

    /// Closes the runner process in multi-thread mode.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the process is closed successfully, or an `Error` if an issue occurs.
    ///
    /// # Note
    ///
    /// This method should only be used for multi-thread mode.
    pub fn close_runner_process(&mut self) -> Result<(), Error> {
        if (self.stellar_core_runner.thread_mode())
            == Some(&BufferedLedgerMetaReaderMode::SingleThread)
        {
            return Err(Error::CloseOnSingleThread);
        }

        Ok(self.stellar_core_runner.close_runner()?)
    }

    /// Retrieves the ledger metadata for a specific ledger sequence.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The ledger sequence number to retrieve metadata for.
    ///
    /// # Returns
    ///
    /// Returns the `LedgerCloseMeta` if found, or an `Error` if the ledger is not found.
    pub fn get_ledger(&self, sequence: u32) -> Result<LedgerCloseMeta, Error> {
        let prepared = self.stellar_core_runner.read_prepared();

        for ledger in prepared {
            if let Some(wrapper) = ledger.ledger_close_meta {
                let meta = wrapper.ledger_close_meta;
                let ledger_seq = match meta.clone() {
                    LedgerCloseMeta::V1(v1) => v1.ledger_header.header.ledger_seq,
                    LedgerCloseMeta::V0(v0) => v0.ledger_header.header.ledger_seq,
                };

                if ledger_seq == sequence {
                    return Ok(meta);
                }
            }
        }

        Err(Error::LedgerNotFound)
    }

    pub async fn async_prepare_ledgers(&mut self, range: &Range, to_current: bool) -> Result<tokio::sync::mpsc::UnboundedReceiver<Box<MetaResult>>, Error> {
        match range {
            Range::Bounded(range) => {
                self.stellar_core_runner.async_catchup_multi_thread(range.0, range.1, to_current).await.map_err(|runner| Error::Core(runner))
            }
        }
    }

    /// Starts the runner in online mode without specifying a range.
    ///
    /// # Returns
    ///
    /// Returns a channel receiver for receiving metadata results if the runner starts successfully,
    /// or an `Error` if an issue occurs.
    pub fn start_online_no_range(&mut self) -> Result<Receiver<Box<MetaResult>>, Error> {
        Ok(self.stellar_core_runner.run()?)
    }


    pub async fn async_start_online_no_range(&mut self) -> Result<tokio::sync::mpsc::UnboundedReceiver<Box<MetaResult>>, Error> {
        Ok(self.stellar_core_runner.run_async().await?)
    }

    // TODO: method to start from ledger.
}
