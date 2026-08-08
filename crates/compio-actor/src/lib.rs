#![doc = include_str!("../README.md")]
#![deny(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]

pub mod actor;
pub mod cluster;
pub mod mailbox;
pub mod process_group;
pub mod supervisor;

pub use actor::{Actor, ActorExit, ActorHandle, Handler, Message};
pub use cluster::Cluster;
pub use mailbox::{Broker, Call, Mailbox, Reply};
pub use process_group::{ProcessGroup, RoundRobin, Strategy};
pub use supervisor::{SupervisionEvent, Supervisor};
