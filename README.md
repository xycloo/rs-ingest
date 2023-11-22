[![Crates.io](https://img.shields.io/crates/v/ingest.svg)](https://crates.io/crates/ingest)

# rs-ingest
## Ingestion library written in rust for Futurenet

- [rs-ingest](#rs-ingest)
  * [Ingestion library written in rust for Futurenet](#ingestion-library-written-in-rust-for-futurenet)
- [Features](#features)
  * [Running offline](#running-offline)
    + [Single-thread mode](#single-thread-mode)
    + [Multi-thread mode](#multi-thread-mode)
  * [Running online](#running-online)
    + [Multi-threaded mode](#multi-threaded-mode)
  * [Closing mechanism](#closing-mechanism)
- [Try it out](#try-it-out)
    + [stellar-core setup](#stellar-core-setup)
- [Learn](#learn)

This package provides primitives for building custom ingestion engines on Futurenet. It's inspired from stellars [`go/ingest`](https://github.com/stellar/go/tree/master/ingest) package.

Often, developers either need ingestion features that are outside of Horizon's scope, or need higher availability for the data. For example, a protocol's frontend might need to ingest events into their own database with a very specific filtering or at a large amount, or they might also need to replay history to see if they missed some events. 

This crate is being designed with this need in mind, and works on futurenet!

> Note: You can also use this crate for pubnet, see the [example](./src/bin/hello_pubnet.rs).

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

The crate is available on crates.io:

```rust
ingest = "0.0.3"
```

### stellar-core setup

Before using the crate, you need the [`stellar-core`](https://github.com/stellar/stellar-core) executable. To install the currently futurenet-compatible core:

```
git clone https://github.com/stellar/stellar-core

cd stellar-core

git checkout v20.0.0rc1

git submodule init

git submodule update

./autogen.sh

CXX=clang++-12 ./configure --enable-next-protocol-version-unsafe-for-production

make

make install [this one might need root access on some machines]
```

> Note. Depending on the machine, you might need a different cmake env var than `CXX=clang++-12`.


# Learn

Check out [LEARN.md](./LEARN.md)
