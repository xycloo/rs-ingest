//! rs-ingest
//!
//! This crate provides primitives for building custom ingestion engines on Futurenet using Rust.
//! It's inspired by the [`go/ingest`](https://github.com/stellar/go/tree/master/ingest) package from the Stellar repository.
//!
//! Developers often require ingestion features that are outside the scope of Horizon's capabilities or
//! need higher availability for data processing. This crate aims to cater to such needs by providing the
//! means to build custom ingestion mechanisms tailored to specific requirements.
//!
//! > Note: This crate is still a work in progress. Its current capabilities are limited.
//!
//! > Note: Only POSIX systems are currently supported.
//!
//! # Features
//!
//! The crate offers the following features:
//!
//! ## Running Offline
//! Running offline involves replaying historical data through a catchup process. This crate supports two offline modes:
//!
//! - Single-thread mode: Waits for the core subprocess to finish catching up and allows retrieval of ledger metadata.
//! - Multi-thread mode: Returns a `Receiver<MetaResult>` that receives newly decoded ledger metadata as it becomes available.
//!
//! ## Running Online
//! Running online involves syncing with Futurenet and receiving ledger close metadata in real-time. This mode is useful for
//! tasks requiring real-time data ingestion, such as event streaming.
//!
//! > Note: Currently, online mode only supports multi-threaded execution.
//!
//! To learn more about the crate and check out a couple of examples see the [README](https://github.com/xycloo/rs-ingest/blob/main/README.md)
//! 

#![warn(missing_docs)]
mod buffered_ledger_meta_reader;
mod ingestion_config;
mod core_runner;
mod captive_core;
mod toml;

pub use buffered_ledger_meta_reader::*;
pub use ingestion_config::*;
pub use core_runner::*;
pub use captive_core::*;
