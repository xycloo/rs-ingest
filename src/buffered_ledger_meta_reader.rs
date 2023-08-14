use std::io::{self, Read};
use std::sync::{Arc, Mutex};
use stellar_xdr::next::{TypeVariant, LedgerCloseMeta, Type};

// from the stellar/go/ingestion lib
const META_PIPE_BUFFER_SIZE: usize = 10 * 1024 * 1024;
const LEDGER_READ_AHEAD_BUFFER_SIZE: usize = 20;

#[derive(thiserror::Error, Debug, Clone )]

pub enum BufReaderError {
    #[error("unknown type {0}, choose one of {1:?}")]
    UnknownType(String, &'static [&'static str]),

    #[error("error decoding XDR")]
    ReadXdrNext,

    #[error("wants to run single-threaded mode but specified transmitter")]
    UnusedTransmitter,

    #[error("wants to run multi-threaded mode but no transmitter specified")]
    MissingTransmitter,

    #[error("wants to use single-threaded mode features but is multi-thread mode")]
    WrongModeMultiThread,

    #[error("wants to use multi-threaded mode features but is single-thread mode")]
    WrongModeSingleThread,

    #[error("cloned bufreaders must only be used for their thread mode")]
    UsedClonedBufreader
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

#[derive(PartialEq, Eq, Clone)]
pub enum BufferedLedgerMetaReaderMode {
    SingleThread,
    MultiThread,
}

pub struct BufferedLedgerMetaReader {
    mode: BufferedLedgerMetaReaderMode,
    reader: Option<io::BufReader<Box<dyn Read + Send>>>,
    cached: Option<Arc<Mutex<Vec<MetaResult>>>>,
    transmitter: Option<std::sync::mpsc::Sender<MetaResult>>,
    cloned: bool
}

impl Clone for BufferedLedgerMetaReader {
    fn clone(&self) -> Self {
        Self { mode: self.mode.clone(), reader: None, cached: None, transmitter: None, cloned: true }
    }
}

impl BufferedLedgerMetaReader {
    pub fn new(mode: BufferedLedgerMetaReaderMode, reader: Box<dyn Read + Send>, transmitter: Option<std::sync::mpsc::Sender<MetaResult>>) -> Result<Self, BufReaderError> {
        let reader = io::BufReader::with_capacity(META_PIPE_BUFFER_SIZE, reader);
        let (cached, transmitter) = match mode {
            BufferedLedgerMetaReaderMode::SingleThread => {
                if transmitter.is_some() {
                    return Err(BufReaderError::UnusedTransmitter);
                }

                (Some(Arc::new(Mutex::new(Vec::with_capacity(LEDGER_READ_AHEAD_BUFFER_SIZE)))), None)
            },
            BufferedLedgerMetaReaderMode::MultiThread => {
                if transmitter.is_none() {
                    return Err(BufReaderError::MissingTransmitter);
                }

                (None, transmitter)
            },
        };

        Ok(
            Self { 
                mode,
                reader: Some(reader), 
                cached,
                transmitter, 
                cloned: false
            }
        )
    }

    pub fn thread_mode(&self) -> &BufferedLedgerMetaReaderMode {
        &self.mode
    }
}

pub trait SingleThreadBufferedLedgerMetaReader {

    fn single_thread_read_ledger_meta_from_pipe(&mut self) -> Result<(), BufReaderError>;
    
    fn read_meta(&self) -> Result<Vec<MetaResult>, BufReaderError>;

    fn clear_buffered(&mut self) -> Result<(), BufReaderError>;
}

pub trait MultiThreadBufferedLedgerMetaReader {

    fn multi_thread_read_ledger_meta_from_pipe(&mut self) -> Result<(), BufReaderError>;    
}


impl SingleThreadBufferedLedgerMetaReader for BufferedLedgerMetaReader {
    fn single_thread_read_ledger_meta_from_pipe(&mut self) -> Result<(), BufReaderError> {
        if self.mode != BufferedLedgerMetaReaderMode::SingleThread {
            return Err(BufReaderError::WrongModeMultiThread)
        }

        if self.cloned {
            return Err(BufReaderError::UsedClonedBufreader)
        }

        for t in stellar_xdr::next::Type::read_xdr_framed_iter(TypeVariant::LedgerCloseMeta, &mut self.reader.as_mut().unwrap()) {
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
            
            // The blow unwrap on cached is safe since initialization
            // prevents initializing in the wrong mode and all
            // BufferedLedgerMetaReader fields are private.
            self.cached.as_ref().unwrap().lock().unwrap().push(meta_obj);
        }

        Ok(())
    }

    fn read_meta(&self) -> Result<Vec<MetaResult>, BufReaderError> {
        if self.mode != BufferedLedgerMetaReaderMode::SingleThread {
            return Err(BufReaderError::WrongModeMultiThread)
        }

        if self.cloned {
            return Err(BufReaderError::UsedClonedBufreader)
        }

        // The blow unwrap on cached is safe since initialization
        // prevents initializing in the wrong mode and all
        // BufferedLedgerMetaReader fields are private.
        let locked = self.cached.as_ref().unwrap().lock().unwrap();
        Ok((*locked).clone())
    }

    fn clear_buffered(&mut self) -> Result<(), BufReaderError> {
        if self.mode != BufferedLedgerMetaReaderMode::SingleThread {
            return Err(BufReaderError::WrongModeMultiThread)
        }

        if self.cloned {
            return Err(BufReaderError::UsedClonedBufreader)
        }

        self.cached = Some(Arc::new(Mutex::new(Vec::with_capacity(LEDGER_READ_AHEAD_BUFFER_SIZE))));
        Ok(())
    }

}


impl MultiThreadBufferedLedgerMetaReader for BufferedLedgerMetaReader {
    fn multi_thread_read_ledger_meta_from_pipe(&mut self) -> Result<(), BufReaderError> {
        if self.mode != BufferedLedgerMetaReaderMode::MultiThread {
            return Err(BufReaderError::WrongModeSingleThread)
        }

        if self.cloned {
            return Err(BufReaderError::UsedClonedBufreader)
        }

        for t in stellar_xdr::next::Type::read_xdr_framed_iter(TypeVariant::LedgerCloseMeta, &mut self.reader.as_mut().unwrap()) {
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
            
            // The blow unwrap on the transmitter is safe since
            // initialization prevents initializing in the wrong mode
            // and all BufferedLedgerMetaReader fields are private.
            self.transmitter.as_ref().unwrap().send(meta_obj).unwrap();
        }

        Ok(())
    }
}
