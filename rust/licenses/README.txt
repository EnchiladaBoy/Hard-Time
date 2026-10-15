Rust dependency notices
=======================

scripts/package-rust.py collects license, copyright, notice and font-license
files from the Cargo packages resolved for Windows, including their nested
resources. It also includes the Rust standard library's copyright record.

Some published crates omit their repository-level license files. This
directory supplies those files from the exact upstream revision recorded
in each crate's .cargo_vcs_info.json. upstream.json records their source
URLs and SHA256 hashes. The package script checks these hashes before use.
These texts apply to the named dependencies, not to the original game.

constgebra 0.1.4 declares MIT OR Apache-2.0 in its Cargo.toml but supplies no
separate license text in the crate or pinned repository. Packaging selects
Apache-2.0 for that crate and includes the standard Apache-2.0 text together
with its original manifest and README. Its const_soft_float dependency's
own notices are collected separately.

The package's dependency index lists versions, declared licenses, source
repositories and the preserved files. It conservatively includes build
dependencies as well as runtime dependencies.
