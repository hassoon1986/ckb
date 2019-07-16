//! # The Chain Library
//!
//! This Library contains the `ChainProvider` traits and `Chain` implement:
//!
//! - [ChainProvider](chain::chain::ChainProvider) provide index
//!   and store interface.
//! - [Chain](chain::chain::Chain) represent a struct which
//!   implement `ChainProvider`

pub mod chain;
mod proposal_table;

#[cfg(test)]
mod tests;

pub(crate) const LOG_TARGET_CHAIN: &str = "ckb-chain";
