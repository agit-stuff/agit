# Contributing to AGIT

Thank you for your interest in contributing to AGIT! This document provides guidelines and information for contributors.

## Development Setup

### Prerequisites

- Rust 1.75 or later
- Git

### Building from Source

```bash
# Clone the repository
git clone https://github.com/agit-stuff/agit.git
cd agit

# Build
cargo build

# Run tests
cargo test

# Run with debug output
RUST_LOG=debug cargo run -- <command>
```

### Running Checks

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Run all tests
cargo test --all-features

# Generate documentation
cargo doc --open
```

## Project Structure

```
src/
├── main.rs          # CLI entry point
├── lib.rs           # Library exports
├── cli/             # CLI commands
├── core/            # Business logic
├── domain/          # Data models
├── storage/         # Persistence layer
├── git/             # Git integration
├── mcp/             # MCP server
├── templates/       # AI instruction templates
├── safety/          # Atomic writes, locking
└── error/           # Error types
```

## Code Style

- Follow the Rust API Guidelines
- Use `rustfmt` for formatting
- Address all `clippy` warnings
- Write documentation for public APIs
- Include tests for new functionality

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests and linting (`cargo test && cargo clippy`)
5. Commit with a clear message
6. Push to your fork
7. Open a Pull Request

### Commit Messages

Use clear, descriptive commit messages:

```
feat: Add support for custom roadmap files

- Implement roadmap command for viewing/editing
- Add roadmap blob type
- Update commit pipeline to use custom roadmap
```

Prefixes:
- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation
- `refactor:` Code refactoring
- `test:` Test additions/changes
- `chore:` Maintenance tasks

## Testing

- Write unit tests for all new functions
- Add integration tests for CLI commands
- Test edge cases and error conditions
- Aim for 80%+ code coverage

## Documentation

- Document all public APIs with rustdoc
- Include examples in documentation
- Update README.md for user-facing changes
- Add architecture docs for significant changes

## Questions?

- Open an issue for bugs or feature requests
- Use discussions for questions
- Join our community chat (coming soon)

## License

By contributing, you agree that your contributions will be licensed under the same terms as the project (MIT OR Apache-2.0).
