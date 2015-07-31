# rustdoc.org

This is my prototype of a rustdoc.org

## What it does

First, execute `cargo run`.

Go to `localhost:1789/github.com/rust-lang/num/num/index.html`. Wait a second,
then boom! The documentation for the num crate appears before your eyes.

When you visit any directory underneath `/github.com/:username/:repo`, the
server will download that repo from GitHub, run `cargo doc` on it, and serve the
results. That's why the first request is pretty slow, but afterwords it's just
serving static files.

## Future improvements

- Talk to crates.io directly?
- Support multiple versions for a crate (crate version, git tag, git branch?)
- Search for a crate
- Show something while generating docs
