use std::{process::{Command, Child}, fmt::format, io::BufReader};
use crate::{IngestionConfig, BufferedLedgerMetaReader, BufferedLedgerMetaReaderPublic, BufReaderError, MetaResult};
use std::io::{BufRead, Error, ErrorKind};


#[derive(PartialEq, Eq)]
enum RunnerStatus {
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

    fn load_prepared(&mut self) {
        let ledgers_meta = self.ledger_buffer_reader.as_ref().unwrap().read_meta();
        self.prepared = Some(ledgers_meta);
    }
}

pub trait StellarCoreRunnerPublic {
    fn new(config: IngestionConfig) -> Self;

    fn catchup(&mut self, from: u32, to: u32) -> Result<(), RunnerError>;

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

    fn catchup(&mut self, from: u32, to: u32)-> Result<(), RunnerError> {
        if self.status != RunnerStatus::Closed {
            return Err(RunnerError::AlreadyRunning)
        }

        self.status = RunnerStatus::RunningOffline;

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

        self.ledger_buffer_reader = Some(BufferedLedgerMetaReader::new(Box::new(reader)));
        
        self.ledger_buffer_reader.as_mut().unwrap().read_ledger_meta_from_pipe()?;

        // Once the stream is finished we can switch back to closed.
        self.status = RunnerStatus::Closed;

        self.load_prepared();
        
        Ok(())
    }

    fn read_prepared(&self) -> Vec<MetaResult> {
        self.prepared.as_ref().unwrap().clone()
    }

}


