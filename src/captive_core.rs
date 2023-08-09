use std::ops::RangeBounds;

use stellar_xdr::{next::{LedgerCloseMeta, LedgerCloseMetaV1}};

use crate::{StellarCoreRunner, IngestionConfig, StellarCoreRunnerPublic, RunnerError, toml::generate_predefined_cfg};

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
    #[error("error while running catchup: {0}")]
    Catchup(#[from] RunnerError),

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


    fn offline_replay(&mut self, from: u32, to: u32) -> Result<(), Error> {
        // TODO: get archiver last checkpoint ledger for error accuracy.

        self.stellar_core_runner.catchup(from, to)?;

        Ok(())
    }

    pub fn prepare_ledgers(&mut self, range: &Range) -> Result<(), Error> {
        match range {
            Range::Bounded(range) => {
                self.offline_replay(range.0, range.1)?;
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
                
                //let json = serde_json::to_string(&meta).expect("serialization error");
                //let deser: LedgerCloseMetaV1 = serde_json::from_str(&json).unwrap();

                if ledger_seq == sequence {
                    return Ok(meta)
                }
            }
        }

        Err(Error::LedgerNotFound)
    }
}
