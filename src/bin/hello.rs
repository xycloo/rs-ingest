use ingest::{IngestionConfig, CaptiveCore, Range, BoundedRange};
use stellar_xdr::next::LedgerCloseMeta;

pub fn main() {
    let config = IngestionConfig {
        executable_path: "/usr/local/bin/stellar-core".to_string(),
        context_path: Default::default(),
    };

    let mut captive_core = CaptiveCore::new(config);

    let range = Range::Bounded(BoundedRange(292395, 292396));
    captive_core.prepare_ledgers_single_thread(&range).unwrap();

    let ledger = captive_core.get_ledger(292395);
    let ledger_seq = match ledger.unwrap() {
        LedgerCloseMeta::V1(v1) => v1.ledger_header.header.ledger_seq,
        _ => unreachable!()
    };

    println!("Hello ledger {}", ledger_seq);
}
