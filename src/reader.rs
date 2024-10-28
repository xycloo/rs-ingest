use stellar_xdr::next::{LedgerCloseMeta, TransactionEnvelope, GeneralizedTransactionSet, TransactionPhase, TxSetComponent, TransactionResultMeta, SorobanTransactionMeta, TransactionMeta, ContractEvent};

use crate::{MetaResult, BufReaderError};

#[derive(thiserror::Error, Debug, Clone)]
pub enum ReaderError {
    #[error("Error while reading meta result {0}")]
    MetaResult(BufReaderError)

}

pub struct LedgerCloseMetaReader;

impl LedgerCloseMetaReader {

    pub fn ledegr_sequence(result: &MetaResult) -> Result<u32, ReaderError> {
        let meta = MetaResultReader::read_meta(result)?;

        match meta {
            LedgerCloseMeta::V0(v0) => {
                Ok(v0.ledger_header.header.ledger_seq)
            }
            LedgerCloseMeta::V1(v1) => {
                Ok(v1.ledger_header.header.ledger_seq)
            }
            
        }
    }

    pub fn ledger_hash(result: &MetaResult) -> Result<[u8; 32], ReaderError> {
        let meta = MetaResultReader::read_meta(result)?;

        match meta {
            LedgerCloseMeta::V0(v0) => {
                Ok(v0.ledger_header.hash.0)
            }
            LedgerCloseMeta::V1(v1) => {
                Ok(v1.ledger_header.hash.0)
            }
            
        }
    }

    pub fn previous_ledger_hash(result: &MetaResult) -> Result<[u8; 32], ReaderError> {
        let meta = MetaResultReader::read_meta(result)?;

        match meta {
            LedgerCloseMeta::V0(v0) => {
                Ok(v0.ledger_header.header.previous_ledger_hash.0)
            }
            LedgerCloseMeta::V1(v1) => {
                Ok(v1.ledger_header.header.previous_ledger_hash.0)
            }
            
        }
    }

    pub fn protocol_version(result: &MetaResult) -> Result<u32, ReaderError> {
        let meta = MetaResultReader::read_meta(result)?;

        match meta {
            LedgerCloseMeta::V0(v0) => {
                Ok(v0.ledger_header.header.ledger_version)
            }
            LedgerCloseMeta::V1(v1) => {
                Ok(v1.ledger_header.header.ledger_version)
            }
           
        }
    }

    pub fn bucket_list_hash(result: &MetaResult) -> Result<[u8; 32], ReaderError> {
        let meta = MetaResultReader::read_meta(result)?;

        match meta {
            LedgerCloseMeta::V0(v0) => {
                Ok(v0.ledger_header.header.bucket_list_hash.0)
            }
            LedgerCloseMeta::V1(v1) => {
                Ok(v1.ledger_header.header.bucket_list_hash.0)
            }
           
        }
    }

    pub fn count_transactions(result: &MetaResult) -> Result<usize, ReaderError> {
        let meta = MetaResultReader::read_meta(result)?;

        match meta {
            LedgerCloseMeta::V0(v0) => {
                Ok(v0.tx_processing.len())
            }
            LedgerCloseMeta::V1(v1) => {
                Ok(v1.tx_processing.len())
            }
          
        }
    }

    pub fn transaction_envelopes(result: &MetaResult) -> Result<Vec<TransactionEnvelope>, ReaderError> {
        let meta = MetaResultReader::read_meta(result)?;

        match meta {
            LedgerCloseMeta::V0(v0) => Ok(v0.tx_set.txs.to_vec()),
            LedgerCloseMeta::V1(v1) => {
                let mut envelopes = Vec::with_capacity(Self::count_transactions(result)?);
                 
                match &v1.tx_set {
                    GeneralizedTransactionSet::V1(v1) => {
                        for phase in v1.phases.iter() {
                            match phase {
                                TransactionPhase::V0(v0) => {
                                    for component in v0.iter() {
                                        match component {
                                            TxSetComponent::TxsetCompTxsMaybeDiscountedFee(txset) => {
                                                envelopes.append(&mut txset.txs.to_vec())
                                            }
                                        }
                                    }
                                }

                                TransactionPhase::V1(v1) => {
                                    for stage in v1.execution_stages.to_vec() {
                                        for thread in stage.0.to_vec() {
                                            envelopes.append(&mut thread.0.to_vec());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                }
                Ok(envelopes)
            }
          
        }
    }

    pub fn transaction_metas(result: &MetaResult) -> Result<Vec<TransactionResultMeta>, ReaderError> {
        let meta = MetaResultReader::read_meta(result)?;

        match meta {
            LedgerCloseMeta::V0(v0) => Ok(v0.tx_processing.to_vec()),
            LedgerCloseMeta::V1(v1) => Ok(v1.tx_processing.to_vec()),
        }
    }

    pub fn soroban_metas(result: &MetaResult) -> Result<Vec<SorobanTransactionMeta>, ReaderError> {
        let mut soroban_metas = Vec::new();

                for result_meta in Self::transaction_metas(result)? {
                    match result_meta.tx_apply_processing {
                        TransactionMeta::V0(_) => {
                            ()
                        }

                        TransactionMeta::V1(_) => {
                            ()
                        }

                        TransactionMeta::V2(_) => {
                            ()
                        }

                        TransactionMeta::V3(v3) => {
                            if let Some(soroban_meta) = v3.soroban_meta {
                                soroban_metas.push(soroban_meta)
                            }
                        }
                    }
                }

        Ok(soroban_metas)
    }

    pub fn soroban_events(result: &MetaResult) -> Result<Vec<ContractEvent>, ReaderError> {
        let soroban_metas = Self::soroban_metas(result)?;
        let mut contract_events = Vec::new();
        for meta in soroban_metas {
            contract_events.append(&mut meta.events.to_vec())
        }

        Ok(contract_events)
    }

}

pub struct MetaResultReader;

impl MetaResultReader {
    pub fn read_meta(result: &MetaResult) -> Result<&LedgerCloseMeta, ReaderError> {
        if let Some(meta) = &result.ledger_close_meta {
            Ok(&meta.ledger_close_meta)
        } else {
            Err(ReaderError::MetaResult(result.err.clone().unwrap()))
        }
    }
}
