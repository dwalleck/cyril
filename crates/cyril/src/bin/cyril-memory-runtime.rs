use cyril_memory::{RuntimeLaunchConfig, run_runtime};

#[path = "../../../../.cyril-u62f/probe.rs"]
mod phase_probe;
use phase_probe::Phase;

fn main() {
    phase_probe::identity("child_entry");
    let build_phase = Phase::new("child_executor_build");
    let runtime = match build_phase.result(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build(),
    ) {
        Ok(runtime) => runtime,
        Err(error) => panic!("create runtime: {error:?}"),
    };
    let body_phase = Phase::new("child_executor_body");
    runtime.block_on(run_child());
    body_phase.finish("ok");
    let drop_phase = Phase::new("child_executor_drop");
    drop(runtime);
    drop_phase.finish("executor ended");
    phase_probe::event("child_executor_end", ());
}

async fn run_child() {
    let config = match RuntimeLaunchConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            phase_probe::event("child_configuration_error", error);
            phase_probe::event("child_process_exit_skips_executor_drop", 1);
            eprintln!("memory runtime failed");
            std::process::exit(1);
        }
    };

    let runtime_phase = Phase::new("child_runtime");
    if let Err(error) = runtime_phase.result(run_runtime(config).await) {
        phase_probe::event("child_runtime_error", error);
        phase_probe::event("child_process_exit_skips_executor_drop", 1);
        eprintln!("memory runtime failed");
        std::process::exit(1);
    }
}
