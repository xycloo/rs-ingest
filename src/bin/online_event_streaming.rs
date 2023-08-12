use ingest::{IngestionConfig, CaptiveCore};
use stellar_xdr::next::{LedgerCloseMeta, TransactionMeta};

pub fn main() {
    let config = IngestionConfig {
        executable_path: "/usr/local/bin/stellar-core".to_string(),
        context_path: Default::default(),
    };

    let mut captive_core = CaptiveCore::new(config);

    let receiver = captive_core.start_online_no_range().unwrap();

    println!("Capturing all events. When a contract event will be emitted it will be printed to stdout");
    for result in receiver.iter() {
        let ledger = result.ledger_close_meta.unwrap().ledger_close_meta;
        match &ledger {
            LedgerCloseMeta::V1(v1) => {
                for tx_processing in v1.tx_processing.iter() {
                    match &tx_processing.tx_apply_processing {
                        TransactionMeta::V3(meta) => {
                            if let Some(soroban) = &meta.soroban_meta {
                                if !soroban.events.is_empty() {
                                    println!("Events for ledger {}: \n{}\n",  v1.ledger_header.header.ledger_seq, serde_json::to_string_pretty(&soroban.events).unwrap())
                                }
                            }
                        },
                        _ => todo!()
                    }
                }
            },
            _ => ()
        }
    }
}
