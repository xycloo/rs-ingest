use std::{process::{Command, Child}, fmt::format, io::{BufReader, self}, thread::{self, JoinHandle}, sync::{Arc, Mutex, mpsc::Receiver}};
use crate::{IngestionConfig, BufferedLedgerMetaReader, BufReaderError, MetaResult, SingleThreadBufferedLedgerMetaReader, BufferedLedgerMetaReaderMode, MultiThreadBufferedLedgerMetaReader};
use std::io::{BufRead, Error, ErrorKind};


#[derive(PartialEq, Eq)]
pub enum RunnerStatus {
    RunningOffline,
    RunningOnline,
    Closed
}

pub struct StellarCoreRunner {
    //pub configs: IngestionConfig,

    executable_path: String,

    context_path: String,

    status: RunnerStatus,

    ledger_buffer_reader: Option<BufferedLedgerMetaReader>,

    prepared: Option<Vec<MetaResult>>

}

#[derive(thiserror::Error, Debug, Clone)]
pub enum RunnerError {
    #[error("instance of core already running")]
    AlreadyRunning,
    
    #[error("error running CLI command")]
    CliExec,

    #[error("error in reading ledger metadata {0}")]
    MetaReader(#[from] BufReaderError)
}


impl StellarCoreRunner {
    fn run_core_cli(&self, args: &[&str]) -> Result<Child, RunnerError> {
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
            Ok(child) => Ok(child),
            Err(_) => Err(RunnerError::CliExec)
        }
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
}

pub trait StellarCoreRunnerPublic {
    fn new(config: IngestionConfig) -> Self;

    fn catchup_single_thread(&mut self, from: u32, to: u32) -> Result<(), RunnerError>;

    fn catchup_multi_thread(&mut self, from: u32, to: u32) -> Result<Receiver<MetaResult>, RunnerError>;

    fn run(&mut self) -> Result<Receiver<MetaResult>, RunnerError>;

    fn read_prepared(&self) -> Vec<MetaResult>;
}

impl StellarCoreRunnerPublic for StellarCoreRunner {
    fn new(config: IngestionConfig) -> Self {
        Self {
            executable_path: config.executable_path,
            context_path: config.context_path.0,
            status: RunnerStatus::Closed,
            ledger_buffer_reader: None,
            prepared: None
        }
    }

    fn catchup_single_thread(&mut self, from: u32, to: u32)-> Result<(), RunnerError> {
        if self.status != RunnerStatus::Closed {
            return Err(RunnerError::AlreadyRunning)
        }

        self.status = RunnerStatus::RunningOffline;

        // Note the below info doesn't apply to catchup.
        // This whole block exists to make sure the starting
        // point for the catchup is correct.
        //
        // Note: currently this process is very unoptimized
        // i.e it always creates a new db and catches up.
        //
        // TODO: read offline info and decide whether the small
        // catchup and db creation are needed.    
        /*{
            let _ = self.run_core_cli(&["new-db"])?
                    .wait()
                    .unwrap();

    
            if from > 2 {
                let _ = self.run_core_cli(&["catchup", &format!("{}/0", from-1)])?
                    .wait()
                    .unwrap();
            } else {
                let _ = self.run_core_cli(&["catchup", "2/0"])?
                    .wait()
                    .unwrap(); // TODO: handle panic
            }
        }*/

        let range = format!("{}/{}", to, to - from + 1);
    
        let stdout = self.run_core_cli(&["catchup", "--in-memory", &range, "--metadata-output-stream fd:1"])?
            .stdout
            .take()
            .unwrap(); // TODO: handle panic

        let reader = BufReader::new(stdout);
        
        //reader
        //.lines()
        //.filter_map(|line| line.ok())
        //.for_each(|line| println!("{}", line));

        let ledger_buffer_reader = match BufferedLedgerMetaReader::new(BufferedLedgerMetaReaderMode::SingleThread, Box::new(reader), None) {
            Ok(reader) => reader,
            Err(error) => return Err(RunnerError::MetaReader(error))
        };
        self.ledger_buffer_reader = Some(ledger_buffer_reader);
        
        self.ledger_buffer_reader.as_mut().unwrap().single_thread_read_ledger_meta_from_pipe()?;

        // Once the stream is finished we can switch back to closed.
        self.status = RunnerStatus::Closed;

        self.load_prepared()?;
        
        Ok(())
    }

    fn catchup_multi_thread(&mut self, from: u32, to: u32)-> Result<Receiver<MetaResult>, RunnerError> {
        if self.status != RunnerStatus::Closed {
            return Err(RunnerError::AlreadyRunning)
        }

        self.status = RunnerStatus::RunningOffline;

        let range = format!("{}/{}", to, to - from + 1);
    
        let stdout = self.run_core_cli(&["catchup", "--in-memory", &range, "--metadata-output-stream fd:1"])?
            .stdout
            .take()
            .unwrap(); // TODO: handle panic

        let reader = BufReader::new(stdout);
        
        let (transmitter, receiver) = std::sync::mpsc::channel();
        
        let handle = {
            let mut stateless_ledger_buffer_reader = match BufferedLedgerMetaReader::new(
                BufferedLedgerMetaReaderMode::MultiThread, 
                Box::new(reader), 
                Some(transmitter)
            ) {
                Ok(reader) => reader,
                Err(error) => return Err(RunnerError::MetaReader(error))
            };
    
            thread::spawn(move || {
                stateless_ledger_buffer_reader.multi_thread_read_ledger_meta_from_pipe().unwrap()
            })
        };

        Ok(receiver)
    }



    fn run(&mut self) -> Result<Receiver<MetaResult>, RunnerError> {
        if self.status != RunnerStatus::Closed {
            return Err(RunnerError::AlreadyRunning)
        }

        self.status = RunnerStatus::RunningOnline;

        // Creating/resetting the DB and a quick catchup. 
        {
            let _ = self.run_core_cli(&["new-db"])?
                        .wait()
                        .unwrap();

            let _ = self.run_core_cli(&["catchup", "current/2"])?
                        .wait()
                        .unwrap();
        }
                    
        let stdout = self.run_core_cli(
        &[
            "run", 
            "--metadata-output-stream fd:1"
            ]
        )?
        .stdout
            .take()
            .unwrap(); // TODO: handle panic;

        let reader = BufReader::new(stdout);

        let (transmitter, receiver) = std::sync::mpsc::channel();
        
        let handle = {
            let mut stateless_ledger_buffer_reader = match BufferedLedgerMetaReader::new(
                BufferedLedgerMetaReaderMode::MultiThread, 
                Box::new(reader), 
                Some(transmitter)
            ) {
                Ok(reader) => reader,
                Err(error) => return Err(RunnerError::MetaReader(error))
            };
    
            thread::spawn(move || {
                stateless_ledger_buffer_reader.multi_thread_read_ledger_meta_from_pipe().unwrap()
            })
        };

        Ok(receiver)
    }

    fn read_prepared(&self) -> Vec<MetaResult> {
        self.prepared.as_ref().unwrap().clone()
    }

}
