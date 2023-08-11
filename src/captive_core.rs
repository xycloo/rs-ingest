use std::{ops::RangeBounds, sync::mpsc::Receiver};

use stellar_xdr::{next::{LedgerCloseMeta, LedgerCloseMetaV1}};

use crate::{StellarCoreRunner, IngestionConfig, StellarCoreRunnerPublic, RunnerError, toml::generate_predefined_cfg, MetaResult};

pub struct ContextPath(pub String);

impl Default for ContextPath {
    fn default() -> Self {
        Self("/tmp/rs_ingestion_temp".to_string())
    }
}

pub struct BoundedRange(pub u32, pub u32);

pub enum Range {
    Bounded(BoundedRange)
}

impl From<Range> for std::ops::Range<u32> {
    fn from(range: Range) -> Self {
        match range {
            Range::Bounded(bounded_range) => bounded_range.0..bounded_range.1,
        }
    }
}

impl Range {
    pub fn bounded(&self) -> (u32, u32) {
        match self {
            Range::Bounded(bounded_range) => (bounded_range.0, bounded_range.1)
        }
    }
}

#[derive(thiserror::Error, Debug, Clone )]

pub enum Error {
    #[error("error while running core: {0}")]
    Core(#[from] RunnerError),

    #[error("requested ledger was not found in prepared ledgers")]
    LedgerNotFound,
}

pub struct CaptiveCore {
    pub stellar_core_runner: StellarCoreRunner
}

impl CaptiveCore {
    pub fn new(config: IngestionConfig) -> Self {
        // generate configs in path
        generate_predefined_cfg(&config.context_path.0);

        Self { stellar_core_runner: StellarCoreRunner::new(config) }
    }


    fn offline_replay_single_thread(&mut self, from: u32, to: u32) -> Result<(), Error> {
        // TODO: get archiver last checkpoint ledger for error accuracy.

        self.stellar_core_runner.catchup_single_thread(from, to)?;

        Ok(())
    }

    fn offline_replay_multi_thread(&mut self, from: u32, to: u32) -> Result<Receiver<MetaResult>, Error> {
        Ok(self.stellar_core_runner.catchup_multi_thread(from, to)?)
    }

    pub fn prepare_ledgers_single_thread(&mut self, range: &Range) -> Result<(), Error> {
        match range {
            Range::Bounded(range) => {
                self.offline_replay_single_thread(range.0, range.1)?;
            }
        };

        Ok(())
    }

    pub fn get_ledger(&self, sequence: u32) -> Result<LedgerCloseMeta, Error> {
        let prepared = self.stellar_core_runner.read_prepared();

        for ledger in prepared {
            if let Some(wrapper) = ledger.ledger_close_meta {
                let meta = wrapper.ledger_close_meta;
                let ledger_seq = match meta.clone() {
                    LedgerCloseMeta::V1(v1) => v1.ledger_header.header.ledger_seq,
                    _ => unreachable!()
                };

                if ledger_seq == sequence {
                    return Ok(meta)
                }
            }
        }

        Err(Error::LedgerNotFound)
    }

    // TODO: method to start from ledger.

    pub fn start_online_no_range(&mut self) -> Result<Receiver<MetaResult>, Error> {
        Ok(self.stellar_core_runner.run()?)
    }
}
