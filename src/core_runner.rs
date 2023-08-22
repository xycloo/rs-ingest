use std::{process::{Command, Child}, io::{BufReader, self}, thread, sync::mpsc::Receiver};
use crate::{IngestionConfig, BufferedLedgerMetaReader, BufReaderError, MetaResult, 
    SingleThreadBufferedLedgerMetaReader, BufferedLedgerMetaReaderMode, 
    MultiThreadBufferedLedgerMetaReader};


    
/// Represents the status of a core runner.
#[derive(PartialEq, Eq)]
pub enum RunnerStatus {
    /// The runner is actively processing tasks while disconnected from the network.
    RunningOffline,

    /// The runner is actively processing tasks and connected to the network.
    RunningOnline,

    /// The runner has been closed and is no longer processing tasks.
    Closed,
}


/// Core runner object.
pub struct StellarCoreRunner {
    //pub configs: IngestionConfig,

    executable_path: String,

    context_path: String,

    status: RunnerStatus,

    ledger_buffer_reader: Option<BufferedLedgerMetaReader>,

    prepared: Option<Vec<MetaResult>>,

    process: Option<Child>,

    bounded_buffer_size: Option<usize>

}


/// Represents the potential errors that can occur during runner operations.
#[derive(thiserror::Error, Debug)]
pub enum RunnerError {
    /// An instance of the core is already running.
    #[error("Instance of core already running")]
    AlreadyRunning,

    /// Error encountered while running a CLI command.
    #[error("Error running CLI command")]
    CliExec,

    /// Error encountered while reading ledger metadata.
    #[error("Error in reading ledger metadata: {0}")]
    MetaReader(#[from] BufReaderError),

    /// The instance of the core has already been closed.
    #[error("Instance of core already closed")]
    AlreadyClosed,

    /// A process-related error occurred.
    #[error("Process error: {0}")]
    Process(#[from] io::Error),

    /// An attempt was made to kill a process, but no process was found.
    #[error("Asked to kill process, but no process was found")]
    ProcessNotFound,
}


impl StellarCoreRunner {
    fn run_core_cli(&mut self, args: &[&str]) -> Result<(), RunnerError> {
        let conf_arg = format!("--conf {}/stellar-core.cfg", self.context_path);

        let mut cmd = Command::new(&self.executable_path);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.current_dir(&self.context_path)
            .arg(conf_arg)
            //.arg("--in-memory") // TODO: manage in-memory or DB running on implementor choice.
            .arg("--ll ERROR");


        let cmd = cmd
            .stdout(std::process::Stdio::piped())
            .spawn();


        match cmd {
            Ok(child) => {
                self.process = Some(child);
                //Ok(child)
                Ok(())
            },
            Err(_) => Err(RunnerError::CliExec)
        }
    }

    fn kill_process(&mut self) -> Result<(), RunnerError> {
        if let Some(child) = self.process.as_mut() {
            child.kill()?;
            self.process = None;

            Ok(())
        } else {
            Err(RunnerError::ProcessNotFound)
        }
    }

    fn remove_temp_data(&self) -> Result<(), RunnerError> {
        let mut cmd = Command::new("rm");
        cmd.arg("-rf").arg("buckets").current_dir(&self.context_path).spawn()?;

        Ok(())
    }

    fn reset_bufreader(&mut self) {
        self.ledger_buffer_reader = None
    }

    fn load_prepared(&mut self) -> Result<(), RunnerError> {
        match self.ledger_buffer_reader.as_ref().unwrap().read_meta() {
            Ok(ledgers_meta) => { 
                self.prepared = Some(ledgers_meta);
                Ok(())
            },
            Err(error) => Err(RunnerError::MetaReader(error))
        }
    }

    // This function is not yet used anywhere in the codebase but might be in the future.
    #[allow(dead_code)]
    pub(crate) fn status(&self) -> &RunnerStatus {
        &self.status
    }

    pub(crate) fn thread_mode(&self) -> &BufferedLedgerMetaReaderMode {
        self.ledger_buffer_reader.as_ref().unwrap().thread_mode()
    }
}

/// Public interface for interacting with the Stellar Core runner.
/// These actions should not be directory called by the implementor.
pub trait StellarCoreRunnerPublic {
    /// Creates a new instance of the runner with the provided configuration.
    fn new(config: IngestionConfig) -> Self;

    /// Performs catchup using a single thread, processing data from the specified range.
    fn catchup_single_thread(&mut self, from: u32, to: u32) -> Result<(), RunnerError>;

    /// Performs catchup using multiple threads, processing data from the specified range.
    /// Returns a channel receiver for receiving metadata results.
    fn catchup_multi_thread(&mut self, from: u32, to: u32) -> Result<Receiver<Box<MetaResult>>, RunnerError>;

    /// Starts the runner and returns a channel receiver for receiving metadata results.
    fn run(&mut self) -> Result<Receiver<Box<MetaResult>>, RunnerError>;

    /// Reads the prepared metadata results from the runner.
    fn read_prepared(&self) -> Vec<MetaResult>;

    /// Closes the runner, freeing any associated resources.
    fn close_runner(&mut self) -> Result<(), RunnerError>;
}


impl StellarCoreRunnerPublic for StellarCoreRunner {
    fn new(config: IngestionConfig) -> Self {
        Self {
            executable_path: config.executable_path,
            context_path: config.context_path.0,
            status: RunnerStatus::Closed,
            ledger_buffer_reader: None,
            prepared: None,
            process: None,
            bounded_buffer_size: config.bounded_buffer_size
        }
    }

    fn catchup_single_thread(&mut self, from: u32, to: u32)-> Result<(), RunnerError> {
        if self.status != RunnerStatus::Closed {
            return Err(RunnerError::AlreadyRunning)
        }

        self.status = RunnerStatus::RunningOffline;

        let range = format!("{}/{}", to, to - from + 1);
    
        self.run_core_cli(&["catchup", "--in-memory", &range, "--metadata-output-stream fd:1"])?;
        let stdout = self.process.as_mut().unwrap().stdout
            .take()
            .unwrap(); // TODO: handle panic

        let reader = BufReader::new(stdout);
        
        //reader
        //.lines()
        //.filter_map(|line| line.ok())
        //.for_each(|line| println!("{}", line));

        let ledger_buffer_reader = match BufferedLedgerMetaReader::new(BufferedLedgerMetaReaderMode::SingleThread, Box::new(reader), None, None) {
            Ok(reader) => reader,
            Err(error) => return Err(RunnerError::MetaReader(error))
        };
        self.ledger_buffer_reader = Some(ledger_buffer_reader);
        
        self.ledger_buffer_reader.as_mut().unwrap().single_thread_read_ledger_meta_from_pipe()?;

        self.load_prepared()?;

        // for single-thread this function is called within the module.
        self.close_runner()?;
        
        Ok(())
    }

    fn catchup_multi_thread(&mut self, from: u32, to: u32)-> Result<Receiver<Box<MetaResult>>, RunnerError> {
        if self.status != RunnerStatus::Closed {
            return Err(RunnerError::AlreadyRunning)
        }

        self.status = RunnerStatus::RunningOffline;

        let range = format!("{}/{}", to, to - from + 1);
    
        self.run_core_cli(&["catchup", "--in-memory", &range, "--metadata-output-stream fd:1"])?;
        let stdout = self.process.as_mut().unwrap().stdout
            .take()
            .unwrap(); // TODO: handle panic

        let reader = BufReader::new(stdout);

        if let Some(bound) = self.bounded_buffer_size {
            let (transmitter, receiver) = std::sync::mpsc::sync_channel(bound);
            let _handle = {
                let mut stateless_ledger_buffer_reader = 
                    match BufferedLedgerMetaReader::new(
                        BufferedLedgerMetaReaderMode::MultiThread, 
                        Box::new(reader), 
                        None,
                        Some(transmitter)
                    ) {
                        Ok(reader) => reader,
                        Err(error) => return Err(RunnerError::MetaReader(error))
                    };
        
                self.ledger_buffer_reader = Some(stateless_ledger_buffer_reader.clone());
    
                thread::spawn(move || {
                    stateless_ledger_buffer_reader.multi_thread_read_ledger_meta_from_pipe().unwrap()
                })
            };

            Ok(receiver)
        } else {
            let (transmitter, receiver) = std::sync::mpsc::channel();
            let _handle = {
                let mut stateless_ledger_buffer_reader = 
                    match BufferedLedgerMetaReader::new(
                        BufferedLedgerMetaReaderMode::MultiThread, 
                        Box::new(reader), 
                        Some(transmitter),
                        None
                    ) {
                        Ok(reader) => reader,
                        Err(error) => return Err(RunnerError::MetaReader(error))
                    };
        
                self.ledger_buffer_reader = Some(stateless_ledger_buffer_reader.clone());
    
                thread::spawn(move || {
                    stateless_ledger_buffer_reader.multi_thread_read_ledger_meta_from_pipe().unwrap()
                })
            };

            Ok(receiver)
        }
    }



    fn run(&mut self) -> Result<Receiver<Box<MetaResult>>, RunnerError> {
        if self.status != RunnerStatus::Closed {
            return Err(RunnerError::AlreadyRunning)
        }

        self.status = RunnerStatus::RunningOnline;

        // Creating/resetting the DB and a quick catchup. 
        // TODO: optimize this process by checking what's the
        // LCL on the existing database instead of always creating
        // a new one and catching up.
        {
            self.run_core_cli(&["new-db"])?;
            self.process.as_mut().unwrap().wait()
                .unwrap();

            let _ = self.run_core_cli(&["catchup", "current/2"]);
            self.process.as_mut().unwrap().wait()
                .unwrap();
        }
                    
        self.run_core_cli(
        &[
            "run", 
            "--metadata-output-stream fd:1"
            ]
        )?;
        let stdout = self.process.as_mut().unwrap().stdout
            .take()
            .unwrap(); // TODO: handle panic;

        let reader = BufReader::new(stdout);

        if let Some(bound) = self.bounded_buffer_size {
            let (transmitter, receiver) = std::sync::mpsc::sync_channel(bound);
            let _handle = {
                let mut stateless_ledger_buffer_reader = 
                    match BufferedLedgerMetaReader::new(
                        BufferedLedgerMetaReaderMode::MultiThread, 
                        Box::new(reader), 
                        None,
                        Some(transmitter)
                    ) {
                        Ok(reader) => reader,
                        Err(error) => return Err(RunnerError::MetaReader(error))
                    };
        
                self.ledger_buffer_reader = Some(stateless_ledger_buffer_reader.clone());
    
                thread::spawn(move || {
                    stateless_ledger_buffer_reader.multi_thread_read_ledger_meta_from_pipe().unwrap()
                })
            };

            Ok(receiver)
        } else {
            let (transmitter, receiver) = std::sync::mpsc::channel();
            let _handle = {
                let mut stateless_ledger_buffer_reader = 
                    match BufferedLedgerMetaReader::new(
                        BufferedLedgerMetaReaderMode::MultiThread, 
                        Box::new(reader), 
                        Some(transmitter),
                        None
                    ) {
                        Ok(reader) => reader,
                        Err(error) => return Err(RunnerError::MetaReader(error))
                    };
        
                self.ledger_buffer_reader = Some(stateless_ledger_buffer_reader.clone());
    
                thread::spawn(move || {
                    stateless_ledger_buffer_reader.multi_thread_read_ledger_meta_from_pipe().unwrap()
                })
            };

            Ok(receiver)
        }
    }

    fn read_prepared(&self) -> Vec<MetaResult> {
        self.prepared.as_ref().unwrap().clone()
    }

    fn close_runner(&mut self) -> Result<(), RunnerError> {
        if self.status == RunnerStatus::Closed {
            return Err(RunnerError::AlreadyRunning)
        }

        self.status = RunnerStatus::Closed;

        self.kill_process()?;
        self.remove_temp_data()?;
        self.reset_bufreader();

        Ok(())
    }

}
