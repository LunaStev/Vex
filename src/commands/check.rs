pub fn check(
    args: &[String],
    runtime: &[std::ffi::OsString],
    selection: &crate::project::Selection,
    messages: &mut crate::messages::Messages,
) -> Result<crate::outcome::Outcome, diagnostic::Error> {
    super::build::build(
        super::build::BuildMode::Check,
        args,
        runtime,
        selection,
        messages,
    )
}
