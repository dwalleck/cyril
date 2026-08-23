use cyril_memory::{RuntimeLaunchConfig, run_runtime};

#[tokio::main]
async fn main() {
    let config = match RuntimeLaunchConfig::from_env() {
        Ok(config) => config,
        Err(_error) => {
            eprintln!("memory runtime failed");
            std::process::exit(1);
        }
    };

    if run_runtime(config).await.is_err() {
        eprintln!("memory runtime failed");
        std::process::exit(1);
    }
}
