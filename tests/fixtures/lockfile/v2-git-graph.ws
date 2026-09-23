{
    version = 2,
    package = [
        {
            name = "alpha", version = "0.1.0", source = "git",
            git = "https://example.invalid/alpha.git", branch = "main",
            commit = "0123456789abcdef0123456789abcdef01234567",
            resolved = ".vex/deps/alpha", dependencies = ["beta"]
        },
        {
            name = "beta", version = "0.1.0", source = "git",
            git = "https://example.invalid/beta.git", tag = "v0.1.0",
            commit = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            resolved = ".vex/deps/beta", dependencies = []
        }
    ]
}
