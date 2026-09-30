use cyril_core::review::{CrtoolPrefix, ShellDialect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argument = std::env::args().nth(1).ok_or("missing smoke mode")?;
    if argument == "crtool" {
        println!("native-prefix-child");
        return Ok(());
    }
    let dialect = match argument.as_str() {
        "posix" => ShellDialect::Posix,
        "fish" => ShellDialect::Fish,
        "pwsh" => ShellDialect::Pwsh,
        "windows-powershell" => ShellDialect::WindowsPowerShell,
        _ => return Err("unknown smoke mode".into()),
    };
    println!("{}", CrtoolPrefix::current(dialect)?.as_str());
    Ok(())
}
