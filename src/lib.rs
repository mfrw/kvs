#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

use std::io::Cursor;
use std::sync::Arc;

use actix_web::middleware;
use actix_web::middleware::Logger;
use actix_web::web::Data;
use actix_web::HttpServer;
use openraft::storage::Adaptor;
use openraft::BasicNode;
use openraft::Config;
use openraft::TokioRuntime;

use crate::app::App;
use crate::network::api;
use crate::network::management;
use crate::network::raft;
use crate::network::Network;
use openraft::Entry;
use openraft_sledstore::ExampleRequest as Request;
use openraft_sledstore::ExampleResponse as Response;
use openraft_sledstore::SledStore;

pub mod app;
pub mod client;
pub mod network;

pub type NodeId = u64;
pub type TypeConfig = openraft_sledstore::TypeConfig;

//openraft::declare_raft_types!(
///// Declare the type configuration for example K/V store.
//pub TypeConfig: D = Request, R = Response, NodeId = NodeId, Node = BasicNode,
//Entry = openraft::Entry<TypeConfig>, SnapshotData = Cursor<Vec<u8>>, AsyncRuntime = TokioRuntime
//);
//openraft::declare_raft_types!(
//pub TypeConfig: D = Request, R = Response, NodeId = NodeId, Node = BasicNode,
//Entry = Entry<TypeConfig>, SnapshotData = Cursor<Vec<u8>>, AsyncRuntime = TokioRuntime
//);

pub type LogStore = Adaptor<TypeConfig, Arc<SledStore>>;
pub type StateMachineStore = Adaptor<TypeConfig, Arc<SledStore>>;
pub type Raft = openraft::Raft<TypeConfig, Network, LogStore, StateMachineStore>;

pub mod typ {
    use openraft::BasicNode;

    use crate::NodeId;
    use crate::TypeConfig;

    pub type RaftError<E = openraft::error::Infallible> = openraft::error::RaftError<NodeId, E>;
    pub type RPCError<E = openraft::error::Infallible> =
        openraft::error::RPCError<NodeId, BasicNode, RaftError<E>>;

    pub type ClientWriteError = openraft::error::ClientWriteError<NodeId, BasicNode>;
    pub type CheckIsLeaderError = openraft::error::CheckIsLeaderError<NodeId, BasicNode>;
    pub type ForwardToLeader = openraft::error::ForwardToLeader<NodeId, BasicNode>;
    pub type InitializeError = openraft::error::InitializeError<NodeId, BasicNode>;

    pub type ClientWriteResponse = openraft::raft::ClientWriteResponse<TypeConfig>;
}

pub async fn start_example_raft_node(node_id: NodeId, http_addr: String) -> std::io::Result<()> {
    // Create a configuration for the raft instance.
    let config = Config {
        heartbeat_interval: 500,
        election_timeout_min: 1500,
        election_timeout_max: 3000,
        ..Default::default()
    };

    let config = Arc::new(config.validate().unwrap());

    // Create a instance of where the Raft data will be stored.
    // let store = Arc::new(Store::default());
    let sstore = sled::open(format!("/tmp/welcome-to-sled-{}", node_id))?;
    let store = SledStore::new(Arc::new(sstore)).await;

    let (log_store, state_machine) = Adaptor::new(store.clone());

    // Create the network layer that will connect and communicate the raft instances and
    // will be used in conjunction with the store created above.
    let network = Network {};

    // Create a local raft instance.
    let raft = openraft::Raft::new(node_id, config.clone(), network, log_store, state_machine)
        .await
        .unwrap();

    // Create an application that will store all the instances created above, this will
    // be later used on the actix-web services.
    let app_data = Data::new(App {
        id: node_id,
        addr: http_addr.clone(),
        raft,
        store,
        config,
    });

    // Start the actix-web server.
    let server = HttpServer::new(move || {
        actix_web::App::new()
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .wrap(middleware::Compress::default())
            .app_data(app_data.clone())
            // raft internal RPC
            .service(raft::append)
            .service(raft::snapshot)
            .service(raft::vote)
            // admin API
            .service(management::init)
            .service(management::add_learner)
            .service(management::change_membership)
            .service(management::metrics)
            // application API
            .service(api::write)
            .service(api::read)
            .service(api::consistent_read)
    });

    let x = server.bind(http_addr)?;

    x.run().await
}
