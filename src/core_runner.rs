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

    prepared: Option<Vec<MetaResult>>,

    process: Option<Child>

}

#[derive(thiserror::Error, Debug)]
pub enum RunnerError {
    #[error("instance of core already running")]
    AlreadyRunning,
    
    #[error("error running CLI command")]
    CliExec,

    #[error("error in reading ledger metadata {0}")]
    MetaReader(#[from] BufReaderError),

    #[error("instance of core already closed")]
    AlreadyClosed,

    #[error("process error {0}")]
    Process(#[from] std::io::Error),

    #[error("Asked to kill process, but no process was found")]
    ProcessNotFound
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

    pub fn status(&self) -> &RunnerStatus {
        &self.status
    }

    pub fn thread_mode(&self) -> &BufferedLedgerMetaReaderMode {
        self.ledger_buffer_reader.as_ref().unwrap().thread_mode()
    }
}

pub trait StellarCoreRunnerPublic {
    fn new(config: IngestionConfig) -> Self;

    fn catchup_single_thread(&mut self, from: u32, to: u32) -> Result<(), RunnerError>;

    fn catchup_multi_thread(&mut self, from: u32, to: u32) -> Result<Receiver<MetaResult>, RunnerError>;

    fn run(&mut self) -> Result<Receiver<MetaResult>, RunnerError>;

    fn read_prepared(&self) -> Vec<MetaResult>;

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
    
        self.run_core_cli(&["catchup", "--in-memory", &range, "--metadata-output-stream fd:1"])?;
        let stdout = self.process.as_mut().unwrap().stdout
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

        self.load_prepared()?;

        // for single-thread this function is called within the module.
        self.close_runner()?;
        
        Ok(())
    }

    fn catchup_multi_thread(&mut self, from: u32, to: u32)-> Result<Receiver<MetaResult>, RunnerError> {
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
    
            self.ledger_buffer_reader = Some(stateless_ledger_buffer_reader.clone());

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
    
            self.ledger_buffer_reader = Some(stateless_ledger_buffer_reader.clone());

            thread::spawn(move || {
                stateless_ledger_buffer_reader.multi_thread_read_ledger_meta_from_pipe().unwrap()
            })
        };

        Ok(receiver)
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
