use ingest::{BoundedRange, BufReaderError, CaptiveCore, IngestionConfig, Range, SupportedNetwork};
use stellar_xdr::next::LedgerCloseMeta;

pub fn main() {
    let config = IngestionConfig {
        executable_path: "/usr/local/bin/stellar-core".to_string(),
        context_path: Default::default(),
        network: SupportedNetwork::Futurenet,
        bounded_buffer_size: None,
        staggered: None,
    };

    let mut captive_core = CaptiveCore::new(config);

    let range = Range::Bounded(BoundedRange(292395, 292396));
    let rx = captive_core.prepare_ledgers_multi_thread(&range).unwrap();

    for ledger in rx.iter() {
        if let Some(meta) = ledger.ledger_close_meta {
            let ledger_seq = match meta.ledger_close_meta {
                LedgerCloseMeta::V1(v1) => v1.ledger_header.header.ledger_seq,
                LedgerCloseMeta::V0(v0) => v0.ledger_header.header.ledger_seq,
                LedgerCloseMeta::V2(v2) => v2.ledger_header.header.ledger_seq,
            };

            // remember that catchup jobs ensure that the requested ledgers
            // are prepared but might also prepare other additional
            // previous ledgers.
            if ledger_seq >= range.bounded().0 && ledger_seq <= range.bounded().1 {
                println!("Hello ledger {}", ledger_seq);
            }
        } else {
            match ledger.err.as_ref().unwrap() {
                BufReaderError::ReadXdrNext => {
                    captive_core.close_runner_process().unwrap();
                    println!("catchup job finished");
                    std::process::exit(0)
                }

                _ => println!(
                    "Error occurred when processing this ledger meta {:?}",
                    ledger.err
                ),
            }
        }
    }
}
