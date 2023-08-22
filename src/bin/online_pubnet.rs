// Note: this example is still untested.

use ingest::{IngestionConfig, CaptiveCore, SupportedNetwork};
use stellar_xdr::next::{LedgerCloseMeta, TransactionMeta};

pub fn main() {
    let config = IngestionConfig {
        executable_path: "/usr/local/bin/stellar-core".to_string(),
        context_path: Default::default(),
        network: SupportedNetwork::Pubnet,
        bounded_buffer_size: None,
        staggered: None
    };

    let mut captive_core = CaptiveCore::new(config);

    let receiver = captive_core.start_online_no_range().unwrap();

    println!("Printing tx sets");
    for result in receiver.iter() {
        let ledger = result.ledger_close_meta.unwrap().ledger_close_meta;
        match &ledger {
            LedgerCloseMeta::V0(v0) => {
                println!("v0 meta: {}", serde_json::to_string_pretty(&v0.tx_set).unwrap())
            },
            LedgerCloseMeta::V1(v1) => {
                println!("v1 meta: {}", serde_json::to_string_pretty(&v1.tx_set).unwrap())
            },
            LedgerCloseMeta::V2(v2) => {
                println!("v2 meta: {}", serde_json::to_string_pretty(&v2.tx_set).unwrap())
            },
        }
    }
}
