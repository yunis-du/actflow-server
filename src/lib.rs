pub mod common;
pub mod config;
pub mod logger;
pub mod runner;
mod server;

mod proto {
    tonic::include_proto!("workflow");
}

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
