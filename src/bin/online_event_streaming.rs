use ingest::{CaptiveCore, IngestionConfig, SupportedNetwork, LedgerCloseMetaReader};
use stellar_xdr::next::{LedgerCloseMeta, TransactionMeta};

const TARGET_SEQ: u32 = 387468;

pub fn main() {
    let config = IngestionConfig {
        executable_path: "/usr/local/bin/stellar-core".to_string(),
        context_path: Default::default(),
        network: SupportedNetwork::Futurenet,
        bounded_buffer_size: None,
        staggered: None,
    };

    let mut captive_core = CaptiveCore::new(config);

    let receiver = captive_core.start_online_no_range().unwrap();

    println!(
        "Capturing all events. When a contract event will be emitted it will be printed to stdout"
    );
    for result in receiver.iter() {
        let ledger_sequence = LedgerCloseMetaReader::ledegr_sequence(&result).unwrap();
        let events = LedgerCloseMetaReader::soroban_events(&result).unwrap();
        println!("Events for ledger {}:\n{}", ledger_sequence, serde_json::to_string(&events).unwrap())
    }
}
