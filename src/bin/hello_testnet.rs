//! This example is the copy of ./hello.rs but running on
//! stellar's public network. The network choice is specified
//! in the ingestion configs.

use ingest::{BoundedRange, CaptiveCore, IngestionConfig, Range, SupportedNetwork};
use stellar_xdr::next::LedgerCloseMeta;

pub fn main() {
    let config = IngestionConfig {
        executable_path: "/usr/local/bin/stellar-core".to_string(),
        context_path: Default::default(),
        network: SupportedNetwork::Testnet,
        bounded_buffer_size: None,
        staggered: None,
    };

    let mut captive_core = CaptiveCore::new(config);

    let range = Range::Bounded(BoundedRange(1844380, 1844381));
    captive_core.prepare_ledgers_single_thread(&range).unwrap();

    let ledger = captive_core.get_ledger(1844381);
    let ledger_seq = match ledger.as_ref().unwrap() {
        LedgerCloseMeta::V1(v1) => v1.ledger_header.header.ledger_seq,
        LedgerCloseMeta::V0(v0) => v0.ledger_header.header.ledger_seq,
        LedgerCloseMeta::V2(v2) => v2.ledger_header.header.ledger_seq,
    };

    println!("{:?}", ledger);

    println!("Hello ledger {}", ledger_seq);
}
