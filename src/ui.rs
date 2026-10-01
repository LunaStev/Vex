pub fn status(action: &str, message: impl AsRef<str>) {
    let _ = diagnostic::output::stderr(format_args!(
        "{action:>12} {}\n",
        source::redact(message.as_ref())
    ));
}
pub fn error(message: impl AsRef<str>) {
    let _ = diagnostic::output::stderr(format_args!(
        "error: {}\n",
        source::redact(message.as_ref())
    ));
}
pub fn warning(message: impl AsRef<str>) {
    let _ = diagnostic::output::stderr(format_args!(
        "warning: {}\n",
        source::redact(message.as_ref())
    ));
}
