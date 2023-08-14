use std::io::{self, Read};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use stellar_xdr::next::{TypeVariant, LedgerCloseMeta, Type};

// from the stellar/go/ingestion lib
const META_PIPE_BUFFER_SIZE: usize = 10 * 1024 * 1024;
const LEDGER_READ_AHEAD_BUFFER_SIZE: usize = 20;

/// Enum to represent different types of errors related to `BufReader` operations.
#[derive(thiserror::Error, Debug, Clone )]
pub enum BufReaderError {
    /// An unknown type was encountered. Choose from the provided list of valid types.
    #[error("Unknown type {0}, choose one of {1:?}")]
    UnknownType(String, &'static [&'static str]),

    /// Error encountered while decoding XDR data.
    #[error("Error decoding XDR")]
    ReadXdrNext,

    /// Attempted to run single-threaded mode with a specified transmitter, which is unused.
    #[error("Wants to run single-threaded mode but specified transmitter")]
    UnusedTransmitter,

    /// Attempted to run multi-threaded mode without specifying a transmitter.
    #[error("Wants to run multi-threaded mode but no transmitter specified")]
    MissingTransmitter,

    /// Attempted to use single-threaded mode features while in multi-threaded mode.
    #[error("Wants to use single-threaded mode features but is multi-thread mode")]
    WrongModeMultiThread,

    /// Attempted to use multi-threaded mode features while in single-threaded mode.
    #[error("Wants to use multi-threaded mode features but is single-thread mode")]
    WrongModeSingleThread,

    /// Cloned `BufReaders` must only be used for their associated thread mode.
    #[error("Cloned BufReaders must only be used for their thread mode")]
    UsedClonedBufreader,
}

/// Wrapper struct to hold the `LedgerCloseMeta` data.
#[derive(Clone)]
pub struct LedgerCloseMetaWrapper {
    /// The ledger close metadata associated with this wrapper.
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

/// Represents the result of processing ledger metadata.
#[derive(Clone)]
pub struct MetaResult {
    /// The ledger close metadata associated with this result.
    pub ledger_close_meta: Option<LedgerCloseMetaWrapper>,

    /// An optional error encountered during processing.
    pub err: Option<BufReaderError>,
}

/// Enum to indicate the mode of operation for `BufferedLedgerMetaReader`.
#[derive(PartialEq, Eq, Clone)]
pub enum BufferedLedgerMetaReaderMode {
    /// The reader operates in single-thread mode.
    SingleThread,

    /// The reader operates in multi-thread mode.
    MultiThread,
}

/// Struct for reading buffered ledger metadata.
pub struct BufferedLedgerMetaReader {
    /// The mode of operation for the reader.
    mode: BufferedLedgerMetaReaderMode,

    /// An optional buffered reader for reading data.
    /// This value is set as an option to allow cloning
    /// a `BufferedLedgerMetaReader` for it to be used
    /// to retrieve the mode.
    reader: Option<io::BufReader<Box<dyn Read + Send>>>,

    /// An optional cached vector of metadata results.
    /// This will only be used when running offline.
    cached: Option<Arc<Mutex<Vec<MetaResult>>>>,

    /// An optional transmitter for sending metadata results.
    /// This will only be used when running online
    transmitter: Option<Sender<MetaResult>>,

    /// Indicates whether the reader has been cloned.
    /// A cloned reader is just a lightweight placeholder
    /// reader which is only used to retrieve the mode.
    /// 
    /// Cloned readers are only used in multi-thread mode.
    cloned: bool,
}

impl Clone for BufferedLedgerMetaReader {
    fn clone(&self) -> Self {
        Self { mode: self.mode.clone(), reader: None, cached: None, transmitter: None, cloned: true }
    }
}

impl BufferedLedgerMetaReader {
    /// Creates a new `BufferedLedgerMetaReader` instance.
    ///
    /// # Arguments
    ///
    /// * `mode` - The mode of operation for the reader.
    /// * `reader` - The boxed reader used for reading data.
    /// * `transmitter` - An optional transmitter for sending metadata results in multi-thread mode.
    ///
    /// # Returns
    ///
    /// Returns a new `BufferedLedgerMetaReader` instance if successful, or a `BufReaderError` if an issue occurs.
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

    /// Retrieves the thread mode of the `BufferedLedgerMetaReader`.
    ///
    /// # Returns
    ///
    /// Returns a reference to the `BufferedLedgerMetaReaderMode` indicating the current thread mode.
    pub fn thread_mode(&self) -> &BufferedLedgerMetaReaderMode {
        &self.mode
    }
}

/// Trait for reading ledger metadata in single-thread mode from a buffered source.
pub trait SingleThreadBufferedLedgerMetaReader {

    /// Reads ledger metadata from the buffered source in single-thread mode.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if reading is successful, or a `BufReaderError` if an issue occurs.
    fn single_thread_read_ledger_meta_from_pipe(&mut self) -> Result<(), BufReaderError>;
    
    /// Reads and retrieves cached ledger metadata in single-thread mode.
    ///
    /// # Returns
    ///
    /// Returns a vector of `MetaResult` if retrieval is successful, or a `BufReaderError` if an issue occurs.
    fn read_meta(&self) -> Result<Vec<MetaResult>, BufReaderError>;

    /// Clears the cached buffered ledger metadata in single-thread mode.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if clearing is successful, or a `BufReaderError` if an issue occurs.
    fn clear_buffered(&mut self) -> Result<(), BufReaderError>;
}

/// Trait for reading ledger metadata in multi-thread mode from a buffered source.
pub trait MultiThreadBufferedLedgerMetaReader {
    /// Reads ledger metadata from the buffered source in multi-thread mode.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if reading is successful, or a `BufReaderError` if an issue occurs.
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

                Err(_) => MetaResult { 
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

                Err(_) => MetaResult { 
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
