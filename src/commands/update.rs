pub fn update(args: &[String]) -> Result<(), diagnostic::Error> {
    super::fetch::dependency_command(true, args)
}
