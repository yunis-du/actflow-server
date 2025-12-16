use std::sync::Arc;

use anyhow::Result;
use clap::{ArgAction, Parser};
use tokio::runtime::Builder;

use actflow_server::{built_info, common::VersionInfo, config::Config, runner};

#[derive(Parser)]
#[clap(name = "omc-north")]
struct Cmd {
    /// Specify config file location
    #[clap(short = 'f', long, default_value = "/etc/actflow-server/actflow-server.yaml")]
    config_file: String,

    /// Display the version
    #[clap(short, long, action = ArgAction::SetTrue)]
    version: bool,
}

const VERSION_INFO: &'static VersionInfo = &VersionInfo {
    name: built_info::PKG_NAME,
    version: built_info::PKG_VERSION,
    branch: built_info::GIT_HEAD_REF,
    commit_hash: built_info::GIT_COMMIT_HASH,
    compiler: built_info::RUSTC_VERSION,
    compile_time: built_info::BUILT_TIME_UTC,
};

fn main() -> Result<()> {
    let cmd = Cmd::parse();

    if cmd.version {
        println!("{}", VERSION_INFO);
        return Ok(());
    }

    let cfg = Config::load_from_file(cmd.config_file);
    match cfg {
        Ok(cfg) => {
            let runtime = Arc::new(
                Builder::new_multi_thread().worker_threads(cfg.async_worker_thread_number.into()).enable_all().build().unwrap(),
            );
            runner::run(cfg, runtime)
        }
        Err(e) => Err(e.into()),
    }
}
