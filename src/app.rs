use std::sync::Arc;

use crate::NodeId;
use crate::Raft;
use openraft::storage::RaftStorage;
use openraft_sledstore::SledStore;

// Representation of an application state. This struct can be shared around to share
// instances of raft, store and more.
pub struct App {
    pub id: NodeId,
    pub addr: String,
    pub raft: Raft,
    pub store: Arc<SledStore>,
    pub config: Arc<openraft::Config>,
}
