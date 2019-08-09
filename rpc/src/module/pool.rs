use crate::error::RPCError;
use ckb_core::transaction::Transaction as CoreTransaction;
use ckb_jsonrpc_types::{Timestamp, Transaction, TxPoolInfo, Unsigned};
use ckb_network::PeerIndex;
use ckb_shared::shared::Shared;
use ckb_sync::SyncSharedState;
use ckb_tx_pool_executor::TxPoolExecutor;
use jsonrpc_core::Result;
use jsonrpc_derive::rpc;
use numext_fixed_hash::H256;
use std::sync::Arc;

#[rpc]
pub trait PoolRpc {
    // curl -d '{"id": 2, "jsonrpc": "2.0", "method":"send_transaction","params": [{"version":2, "deps":[], "inputs":[], "outputs":[]}]}' -H 'content-type:application/json' 'http://localhost:8114'
    #[rpc(name = "send_transaction")]
    fn send_transaction(&self, _tx: Transaction) -> Result<H256>;

    // curl -d '{"params": [], "method": "tx_pool_info", "jsonrpc": "2.0", "id": 2}' -H 'content-type:application/json' http://localhost:8114
    #[rpc(name = "tx_pool_info")]
    fn tx_pool_info(&self) -> Result<TxPoolInfo>;
}

pub(crate) struct PoolRpcImpl {
    sync_shared_state: Arc<SyncSharedState>,
    shared: Shared,
    tx_pool_executor: Arc<TxPoolExecutor>,
}

impl PoolRpcImpl {
    pub fn new(shared: Shared, sync_shared_state: Arc<SyncSharedState>) -> PoolRpcImpl {
        let tx_pool_executor = Arc::new(TxPoolExecutor::new(shared.clone()));
        PoolRpcImpl {
            sync_shared_state,
            shared,
            tx_pool_executor,
        }
    }
}

impl PoolRpc for PoolRpcImpl {
    fn send_transaction(&self, tx: Transaction) -> Result<H256> {
        let tx: CoreTransaction = tx.into();

        let result = self.tx_pool_executor.verify_and_add_tx_to_pool(tx.clone());

        match result {
            Ok(_) => {
                // workaround: we are using `PeerIndex(usize::max)` to indicate that tx hash source is itself.
                let peer_index = PeerIndex::new(usize::max_value());
                let hash = tx.hash().to_owned();
                self.sync_shared_state
                    .tx_hashes()
                    .entry(peer_index)
                    .or_default()
                    .insert(hash.clone());
                Ok(hash)
            }
            Err(e) => Err(RPCError::custom(RPCError::Invalid, e.to_string())),
        }
    }

    fn tx_pool_info(&self) -> Result<TxPoolInfo> {
        let chain_state = self.shared.lock_chain_state();
        let tx_pool = chain_state.tx_pool();
        Ok(TxPoolInfo {
            pending: Unsigned(u64::from(tx_pool.pending_size())),
            proposed: Unsigned(u64::from(tx_pool.proposed_size())),
            orphan: Unsigned(u64::from(tx_pool.orphan_size())),
            total_tx_size: Unsigned(tx_pool.total_tx_size() as u64),
            total_tx_cycles: Unsigned(tx_pool.total_tx_cycles()),
            last_txs_updated_at: Timestamp(chain_state.get_last_txs_updated_at()),
        })
    }
}
