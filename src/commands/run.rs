pub fn run(
    args: &[String],
    messages: &mut crate::messages::Messages,
) -> Result<crate::outcome::Outcome, diagnostic::Error> {
    super::build::build(super::build::BuildMode::Run, args, messages)
}
