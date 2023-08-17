[![Crates.io](https://img.shields.io/crates/v/ingest.svg)](https://crates.io/crates/ingest)

# rs-ingest
## Ingestion library written in rust for Futurenet

- [Features](#features)
  * [Running offline](#running-offline)
    + [Single-thread mode](#single-thread-mode)
    + [Multi-thread mode](#multi-thread-mode)
  * [Running online](#running-online)
    + [Multi-threaded mode](#multi-threaded-mode)
  * [Closing mechanism](#closing-mechanism)
- [Try it out](#try-it-out)
  * [Setup](#setup)
- [Learn](#learn)
  * [Hello ledger!](#hello-ledger)
  * [Invoke host function operations statistics](#invoke-host-function-operations-statistics)
  * [Event streaming with online mode](#event-streaming-with-online-mode)


This package provides primitives for building custom ingestion engines on Futurenet. It's inspired from stellars [`go/ingest`](https://github.com/stellar/go/tree/master/ingest) package.

Often, developers either need ingestion features that are outside of Horizon's scope, or need higher availability for the data. For example, a protocol's frontend might need to ingest events into their own database with a very specific filtering or at a large amount, or they might also need to replay history to see if they missed some events. 

This crate is being designed with this need in mind, and works on futurenet!

> Note: This crate is still a work in progress. The current capabilities of the crate are limited.

> Note: Currently only POSIX systems are supported.

# Features

Currently, you can both replay history and run online. Running online does not currently support running starting to replay history from a given ledger. 

Note that the current implementation is experimental and does not cover all the functionalities that an ingestion crate should, including but not limited to failsafe mechanisms, archiver interaction, custom toml configuration, readers, and overall a more optimized codebase.

## Running offline
Running offline means being able to replay history through a catchup. Replaying history will enable you to process everything that has happened on the network within the specified bounded range. 

`rs-ingest` allows you to run offline in two modes:
- single-thread
- multi-thread

### Single-thread mode
Running single-thread mode is the most straightforward way of using `rs-ingest`. This mode will await for the core subprocess to finish catching up and will then allow to retrieve the ledger(s) metadata.

Running single-thread mode will store every ledger meta.

### Multi-thread mode
Running multi-thread mode is also pretty simple, but returns a `Receiver<MetaResult>` object that receives new ledger meta (already decoded) as soon as it is emitted.

When you run multi-thread mode you will be in charge of how to store the metadata or object derived from it. 

When running multi-thread mode you also need to call the [closing mechanism](#closing-mechanism) manually.

## Running online
Running online means being able to sync with Futurenet and close ledgers, thus receive ledger close meta. This mode is more suitable for building using real-time data (for example event streaming).

### Multi-threaded mode
Running online can only be done in multi-thread mode. You'll receive a `Receiver<MetaResult>` object which receives ledger close meta as stellar-core closes ledgers.

When running multi-thread mode you also need to call the [closing mechanism](#closing-mechanism) manually.

## Closing mechanism
`rs-ingest` has a closing mechanism that deletes temporary buckets created during execution and clears the objects. Closing is important before re-initiating an action on `rs-ingest`. When running single-thread mode, the closing mechanism is triggered within `rs-ingest` modules, however when running multi-thread it's the implementor that must decide when to trigger the closing mechanism.

# Try it out

The crate is a WIP, but you can already start playing around the features it currently offers. For example, check out the [examples](https://github.com/xycloo/rs-soroban-cortex/tree/main/ingest/src/bin).

The crate is avaiable on crates.io:

```rust
ingest = "0.0.1"
```

### stellar-core setup

Before using the crate, you need the [`stellar-core`](https://github.com/stellar/stellar-core) executable. To install the currently futurenet-compatible core:

```
git clone https://github.com/stellar/stellar-core

cd stellar-core

git checkout b7d3a8f8

git submodule init

git submodule update

./autogen.sh

CXX=clang++-12 ./configure --enable-next-protocol-version-unsafe-for-production

make

make install [this one might need root access on some machines]
```


# Learn
- [Hello ledger](#hello-ledger)
- [Operation analytics](#invoke-host-function-operations-statistics)
- [Event streaming with online mode](#event-streaming-with-online-mode)

The `ingest` crate is pretty easy to use, especially currently since the amount of features is limited and there is a predefined TOML configuration.

Anyways, `ingest` depends on the `stellar-core` executable which enables an option to stream ledger metadata to the provided descriptor. `ingest` simply wraps all the commands and stream-reading for you so you can focus on actually writing your own ingestion mechanism.

## Hello ledger!

This first example is very simple, and consists in capturing a specific ledger and make sure that `ingest` worked properly and captured the correct ledger.

```rust
use ingest::{IngestionConfig, CaptiveCore, Range, BoundedRange};
use stellar_xdr::next::LedgerCloseMeta;

pub fn main() {
    let config = IngestionConfig {
        executable_path: "/usr/local/bin/stellar-core".to_string(),
        context_path: Default::default(),
        network: SupportedNetwork::Futurenet
    };

    let mut captive_core = CaptiveCore::new(config);

    let range = Range::Bounded(BoundedRange(292395, 292396));
    captive_core.prepare_ledgers(&range).unwrap();

    let ledger = captive_core.get_ledger(292395);
    let ledger_seq = match ledger.unwrap() {
        LedgerCloseMeta::V1(v1) => v1.ledger_header.header.ledger_seq,
        _ => unreachable!()
    };

    println!("Hello ledger {}", ledger_seq);
}
```

As you can see, this is all pretty simple and straightforward. Whe first setup the captive core instance by providing the path to our `stellar-core` executable and by choosing the default context path (`/tmp/rs_ingestion_temp/`, holds buckets and potentially the db). Then we choose a range of ledgers we want `ingest` to prepare with `CaptiveCore::prepare_ledgers(&mut CaptiveCore, &Range)` so that we can later get them.

In our case, we just chose to load two ledgers and then capture the first one with `CaptiveCore::get_ledger(&CaptiveCore, ledger_sequence)`.

At this point, it's all about using the `stellar_xdr::next::LedgerCloseMeta` object we received with `get_ledger(sequence)`: we assume it's a v1 ledgerclose meta and then get the ledger's sequence from the header and print it out.

#### Running hello ledger

You can now run this with either `cargo run [OPTIONS]` or by compiling to an executable and running it. This should be pretty quick and the result should be:

```
Warning: soroban-env-host-curr is running a pre-release version 0.0.17
Hello ledger 292395
```

> Note: Don't worry about the warning. `stellar-core` is just letting us know we are working on Futurenet, which has not yeat been released as a stable audited release.

If you're seeing a different result and are experiencing errors, first make sure you have installed the correct version of `stellar-core` (following [this set of commands](#setup)), and if you still experience errors please open an issue. 


## Invoke host function operations statistics

> Note: this doesn't take into account FeeBump transactions.

Now let's get to a more interesting use of `ingest`. We are going to analyze a set of 10000 ledgers and see how many operations that happened within those ledgers are invoke host function operations. This statistic is interesting since host functions are invoked for almost everything soroban-related: uploading contracts and invoking contracts. 

Again, we start by preparing the ledgers we're interested in:

```rust
use ingest::{IngestionConfig, CaptiveCore, Range, BoundedRange};
use stellar_xdr::next::{LedgerCloseMeta, TransactionPhase, TxSetComponent, TransactionEnvelope, OperationBody};

pub fn main() {
    let config = IngestionConfig {
        executable_path: "/usr/local/bin/stellar-core".to_string(),
        context_path: Default::default(),
        network: SupportedNetwork::Futurenet
    };

    let mut captive_core = CaptiveCore::new(config);

    // preparing just 10000 ledgers for simplicity.
    let range = Range::Bounded(BoundedRange(292_000, 302_000)); 
    
    println!("[+] Preparing ledgers [{} to {}]", range.bounded().0, range.bounded().1);
    captive_core.prepare_ledgers(&range).unwrap();

    // ...    

}
```

As you can see, we're preparing 10000 ledgers. Note that preparing these is not instant and will take a bit more than preparing just the two ledgers of the previous example.

Anyways, now it's time to process the prepared ledgers and count how many of the total operations where invoke host functions operations:

```rust
pub fn main() {
    // ...
    let mut all_other_ops = 0;
    let mut invoke_host_ops = 0;

    for n in std::ops::Range::from(range) {
        let ledger = captive_core.get_ledger(n);
        if let LedgerCloseMeta::V1(v1) = ledger.unwrap() {
            let set = match &v1.tx_set {
                stellar_xdr::next::GeneralizedTransactionSet::V1(set) => set
            };
            for tx_phase in set.phases.iter() {
                let set = match tx_phase {
                    TransactionPhase::V0(set) => set
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
                                    },
                                    TransactionEnvelope::TxV0(tx) => {
                                        for op in tx.tx.operations.iter() {
                                            ops.push(op);
                                        }
                                    },
                                    _ => todo!()
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

    println!("Total operations recorded: {}", all_other_ops+invoke_host_ops);
    println!("Non invoke host function operations: {all_other_ops}");
    println!("Invoke host function operations: {invoke_host_ops}");

}
```

Hopefully the above is straightforward, at least in that we're just proecessing everyone of the ledgers we have prepared in the previous step. The processing is done against `stellar_xdr::next::LedgerCloseMeta`, but we realize that as is it's a bit too verbose to get things like operations and so on so we plan to provide some util functions that parse `LedgerCloseMeta` to obtain information. For example, we would like to provide a helper that converts the meta into a `std::Vec` of events.

#### Running

As mentioned previously this one will take a bit more than the hello exampe (~10 minutes).

The result should be the following:

```
[+] Preparing ledgers [292000 to 302000]
Warning: soroban-env-host-curr is running a pre-release version 0.0.17
Total operations recorded: 2760
Non invoke host function operations: 2273
Invoke host function operations: 487
```

This means that one every ~5 and a half operations for the captured ledgers is an invoke host function operation.


## Event streaming with online mode
Running online mode is suitable for tasks that need to ingest real-time data. For example, think of an indexer for contract events. 
It needs to store in a database all events following a certain schema and makes them available by exposing a queriable API. 
The service needs to provide real-time data to its customers, thus will need to run online mode. 

In this example, we explore how we can print out contract events, if any, as ledgers close and the network progresses.

As always, we set up our captive core with the usual configs:

```rust
use ingest::{IngestionConfig, CaptiveCore};
use stellar_xdr::next::{LedgerCloseMeta, TransactionMeta};

const TARGET_SEQ: u32 = 387468;

pub fn main() {
    let config = IngestionConfig {
        executable_path: "/usr/local/bin/stellar-core".to_string(),
        context_path: Default::default(),
        network: SupportedNetwork::Futurenet
    };

    let mut captive_core = CaptiveCore::new(config);

    // ...
}
```

Notice that we added a `TARGET_SEQ` constant. That's because in this example we want to showcase how you could call the [closing mechanism](#closing-mechanism) when running online, and to do so we need a ledger sequence at which our program exists and calls the closing mechanism. 

> Note: when you run this example, choose an appropriate target sequence, for example 100 sequences from the current sequence. 

Now, we need to start our core instance in online mode:

```rust
pub fn main() {
    // ...

    let receiver = captive_core.start_online_no_range().unwrap();

    // ...
}

```

As you can see, we're obtaining a receiver object from starting online. 
The `Receiver<MetaResult>` object is needed for our runner instance thread to communicate with the implementor, i.e you. In fact when you call `captive_core.start_online_no_range()` you are also spawning the stella-core run process and the buffered ledger meta process, which will send meta to the receiver as new framed ledger close meta is decoded. 

Now you can choose how to deal with the receiver, in our case we just loop over it and print out contract events if we find any:

```rust
pub fn main() {
    println!("Capturing all events. When a contract event will be emitted it will be printed to stdout");
    for result in receiver.iter() {
        let ledger = result.ledger_close_meta.unwrap().ledger_close_meta;
        match &ledger {
            LedgerCloseMeta::V1(v1) => {

                let ledger_seq = v1.ledger_header.header.ledger_seq;
                if ledger_seq == TARGET_SEQ {
                    println!("Reached target ledger, closing");
                    captive_core.close_runner_process().unwrap();

                    std::process::exit(0)
                }

                for tx_processing in v1.tx_processing.iter() {
                    match &tx_processing.tx_apply_processing {
                        TransactionMeta::V3(meta) => {
                            if let Some(soroban) = &meta.soroban_meta {
                                if !soroban.events.is_empty() {
                                    println!("Events for ledger {}: \n{}\n",  ledger_seq, serde_json::to_string_pretty(&soroban.events).unwrap())
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
```

As you can see, we're just destructuring every `MetaResult` we receive and print out, if any, the events on the processed ledger.

Notice this code block:

```rust
if ledger_seq == TARGET_SEQ {
    println!("Reached target ledger, closing");
    captive_core.close_runner_process().unwrap();

    std::process::exit(0)
}
```

Where we trigger the closing mechanism and exit successfully.

This might be a good starting point to start working on an indexer for you protocol's contract events.
