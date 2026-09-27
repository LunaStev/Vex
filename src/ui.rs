use std::io::Write;

pub fn status(action: &str, message: impl AsRef<str>) {
    let _ = writeln!(
        std::io::stderr().lock(),
        "{action:>12} {}",
        source::redact(message.as_ref())
    );
}
pub fn error(message: impl AsRef<str>) {
    let _ = writeln!(
        std::io::stderr().lock(),
        "error: {}",
        source::redact(message.as_ref())
    );
}
pub fn warning(message: impl AsRef<str>) {
    let _ = writeln!(
        std::io::stderr().lock(),
        "warning: {}",
        source::redact(message.as_ref())
    );
}
