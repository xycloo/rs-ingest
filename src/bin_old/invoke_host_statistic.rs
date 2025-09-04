use ingest::{BoundedRange, CaptiveCore, IngestionConfig, Range, SupportedNetwork};
use stellar_xdr::next::{
    LedgerCloseMeta, Operation, OperationBody, TransactionEnvelope, TransactionPhase,
    TxSetComponent,
};

pub fn main() {
    let config = IngestionConfig {
        executable_path: "/usr/local/bin/stellar-core".to_string(),
        context_path: Default::default(),
        network: SupportedNetwork::Futurenet,
        bounded_buffer_size: None,
        staggered: None,
    };

    let mut captive_core = CaptiveCore::new(config);

    // preparing just 10000 ledgers for simplicity.
    let range = Range::Bounded(BoundedRange(292_000, 302_000));

    println!(
        "[+] Preparing ledgers [{} to {}]",
        range.bounded().0,
        range.bounded().1
    );
    captive_core.prepare_ledgers_single_thread(&range).unwrap();

    let mut all_other_ops = 0;
    let mut invoke_host_ops = 0;

    for n in std::ops::Range::from(range) {
        let ledger = captive_core.get_ledger(n);
        if let LedgerCloseMeta::V1(v1) = ledger.unwrap() {
            let set = match &v1.tx_set {
                stellar_xdr::next::GeneralizedTransactionSet::V1(set) => set,
            };
            for tx_phase in set.phases.iter() {
                let set = match tx_phase {
                    TransactionPhase::V0(set) => set,
                    _ => panic!("This binary doesn't support v1 txsets yet")
                };
                for set in set.iter() {
                    let ops: Vec<&Operation> = match set {
                        TxSetComponent::TxsetCompTxsMaybeDiscountedFee(set) => {
                            let mut ops = Vec::new();
                            for tx_envelope in set.txs.iter() {
                                match tx_envelope {
                                    TransactionEnvelope::Tx(tx) => {
                                        for op in tx.tx.operations.iter() {
                                            ops.push(op);
                                        }
                                    }
                                    TransactionEnvelope::TxV0(tx) => {
                                        for op in tx.tx.operations.iter() {
                                            ops.push(op);
                                        }
                                    }
                                    _ => todo!(),
                                };
                            }

                            ops
                        }
                    };
                    for op in ops {
                        match op.body {
                            OperationBody::InvokeHostFunction(_) => invoke_host_ops += 1,
                            _ => all_other_ops += 1,
                        }
                    }
                }
            }
        }
    }

    println!(
        "Total operations recorded: {}",
        all_other_ops + invoke_host_ops
    );
    println!("Non invoke host function operations: {all_other_ops}");
    println!("Invoke host function operations: {invoke_host_ops}");

    // print!("Ratio: {}", all_other_ops as f32 / invoke_host_ops as f32);
}
