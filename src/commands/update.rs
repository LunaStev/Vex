use super::fetch::dependency_command;

pub fn update(args: &[String]) {
    dependency_command(true, args);
}
