pub fn update(
    args: &[String],
    selection: &crate::project::Selection,
) -> Result<(), diagnostic::Error> {
    super::fetch::dependency_command(true, args, selection)
}
