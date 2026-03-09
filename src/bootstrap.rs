pub fn imagemagick_probe() -> bool {
    let result = std::process::Command::new("convert").output();
    if let Ok(output) = result {
        output.status.code().unwrap_or(1) != 127
    } else {
        false
    }
}
