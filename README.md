# Vex

Vex is the official package manager and build tool for the Wave programming language. It enables simple, unified project setup and execution using a minimal set of commands. Vex uses WSON (Wave Serialized Object Notation) for managing metadata and configurations.

## Features

- **Simple command-line interface**: Only two main commands to learn
- **Cross-platform foundation**: Future support for cross-building to Linux, Windows, macOS
- **WSON-based project configuration**
- **Seamless integration with the Wave compiler and Whale toolchain**

## Installation

### For Linux:

1. **Download and Extract:**
    - Download the `vex-v0.0.1-pre-alpha-linux.tar.gz` file from the official source.
    - Use the wget command:
      ```bash
      wget https://github.com/LunaStev/Vex/releases/download/v0.0.1-pre-alpha/vex-v0.0.1-pre-alpha-linux.tar.gz
      ```
    - Extract the archive:
      ```bash
      sudo tar -xvzf vex-v0.0.1-pre-alpha-linux.tar.gz -C /usr/local/bin
      ```

2**Verify Installation:**
    - Open a terminal and type:
      ```bash
      vex --version
      ```
    - If the version number displays, the installation was successful.


## Commands

```sh
vex init                # Initialize a new Wave project
vex run [file]          # Run the specified Wave source file
```

## Example Project Structure

```
my_project/
├── src/
│   └── main.wave
├── vex.wson            # Project metadata and configuration
```

## `vex.wson` Example

```wson
{
    name = "my_project",
    version = 0.1.0,
    lib = false,

    // library
    dependencies = []
}
```

## Roadmap

- Support for building projects for different platforms
- Dependency management
- Integration with WPM (Wave Package Manager)
- WSON-powered modular builds

## License

[MPL 2.0 LICENSE](LICENSE)

---

Vex embraces the philosophy of Wave: a unified development experience across low-level and high-level programming tasks, all in one language.

For more information, visit [wave-lang.dev](https://wave-lang.dev)