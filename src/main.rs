use clap::Parser;
use openapi_snapshot::{build_output, validate_config, write_output, AppError, Cli, Config};

fn main() {
    let cli = Cli::parse();
    let config = match Config::from_cli(cli) {
        Ok(config) => config,
        Err(err) => exit_with_error(err),
    };

    if config.stdout && config.out.is_some() {
        eprintln!("--out is ignored because --stdout is set.");
    }

    if let Err(err) = validate_config(&config) {
        exit_with_error(err);
    }

    let payload = match build_output(&config) {
        Ok(payload) => payload,
        Err(err) => exit_with_error(err),
    };

    if let Err(err) = write_output(&config, &payload) {
        exit_with_error(err);
    }
}

fn exit_with_error(err: AppError) -> ! {
    eprintln!("{err}");
    std::process::exit(err.exit_code());
}
