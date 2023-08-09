use std::io::{self, Read};
use std::sync::{Arc, Mutex, MutexGuard};
use serde::Serialize;
use stellar_xdr::next::{TypeVariant, LedgerCloseMeta, LedgerCloseMetaV1, Type};

// from the stellar/go/ingestion lib
const META_PIPE_BUFFER_SIZE: usize = 10 * 1024 * 1024;
const LEDGER_READ_AHEAD_BUFFER_SIZE: usize = 20;

#[derive(thiserror::Error, Debug, Clone )]

pub enum BufReaderError {
    #[error("unknown type {0}, choose one of {1:?}")]
    UnknownType(String, &'static [&'static str]),

    #[error("error decoding XDR")]
    ReadXdrNext,
}

#[derive(Clone)]
pub struct LedgerCloseMetaWrapper {
    pub ledger_close_meta: LedgerCloseMeta
}

impl LedgerCloseMetaWrapper {
    fn new(inner: LedgerCloseMeta) -> Self {
        Self { ledger_close_meta: inner }
    }
}

impl From<Type> for LedgerCloseMetaWrapper {
    fn from(value: Type) -> Self {
        match value {
            Type::LedgerCloseMeta(boxed_ledger_close_meta) => {
                Self::new(*boxed_ledger_close_meta)
            }

            // As long as this code is used for this crate
            // this other match arms should be unreachable.
            //
            // Note: if you want to implement a similar
            // functionality make sure to asses if this
            // unreachable block should be used or not.
            _ => unreachable!()
        }
    }
}

#[derive(Clone)]
pub struct MetaResult {
    pub ledger_close_meta: Option<LedgerCloseMetaWrapper>,
    pub err: Option<BufReaderError>
}

pub struct BufferedLedgerMetaReader {
    pub r: io::BufReader<Box<dyn Read>>,
    pub c: Arc<Mutex<Vec<MetaResult>>>,
}

pub trait BufferedLedgerMetaReaderPublic {
    fn new(reader: Box<dyn Read>) -> Self;

    fn read_ledger_meta_from_pipe(&mut self) -> Result<(), BufReaderError>;
    
    fn read_meta(&self) -> Vec<MetaResult>;

    fn clear_buffered(&mut self);
}


impl BufferedLedgerMetaReaderPublic for BufferedLedgerMetaReader {
    fn new(reader: Box<dyn Read>) -> Self {
        let r = io::BufReader::with_capacity(META_PIPE_BUFFER_SIZE, reader);

        Self { 
            r, 
            c: Arc::new(Mutex::new(Vec::with_capacity(LEDGER_READ_AHEAD_BUFFER_SIZE))) 
        }
    }

    fn read_ledger_meta_from_pipe(&mut self) -> Result<(), BufReaderError> {
        for t in stellar_xdr::next::Type::read_xdr_framed_iter(TypeVariant::LedgerCloseMeta, &mut self.r) {
            let meta_obj = match t {
                Ok(ledger_close_meta) => MetaResult {
                    ledger_close_meta: Some(ledger_close_meta.into()),
                    err: None
                },

                Err(error) => MetaResult { 
                    ledger_close_meta: None, 
                    err: Some(BufReaderError::ReadXdrNext)
                }
            };
            
            self.c.lock().unwrap().push(meta_obj);
        }

        Ok(())
    }

    fn read_meta(&self) -> Vec<MetaResult> {
        let locked = self.c.lock().unwrap();
        (*locked).clone()
    }

    fn clear_buffered(&mut self) {
        self.c = Arc::new(Mutex::new(Vec::with_capacity(LEDGER_READ_AHEAD_BUFFER_SIZE)));
    }

}

