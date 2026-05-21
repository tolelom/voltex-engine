//! Static help text. Kept as string constants (not generated) so the
//! exact output is easy to review and regression-test.

/// Top-level `voltex --help` output.
pub const TOP: &str = "\
voltex — game engine CLI (v1.0.x)

USAGE:
    voltex <command> [options]
    voltex --version
    voltex --help

COMMANDS:
    new      Scaffold a new project in <name>/
    build    Build the project in the current directory
    run      Build then launch the resulting executable

OPTIONS (top-level):
    -h, --help       Print this help
    -V, --version    Print the voltex CLI version

EXAMPLES:
    voltex new my_game
    cd my_game
    voltex build --engine-path /path/to/game_engine
    voltex run --engine-path /path/to/game_engine

Run `voltex <command> --help` for command-specific help.
";

/// `voltex new --help` output.
pub const NEW: &str = "\
voltex new — scaffold a new project

USAGE:
    voltex new <name>

ARGS:
    <name>    Project name. A directory of this name is created in the
              current working directory and is populated with:
                  voltex.toml
                  scene.vscn         (empty scene)
                  scripts/spinner.lua (sample Lua script)
                  assets/.gitkeep    (placeholder)

EXAMPLES:
    voltex new my_game
    cd my_game && voltex build --engine-path /path/to/game_engine

After scaffolding, edit voltex.toml to configure window size, targets,
and asset globs. See `docs/voltex-toml-reference.md` in the engine repo.
";

/// `voltex build --help` output.
pub const BUILD: &str = "\
voltex build — manifest-driven build

USAGE:
    voltex build [options]

OPTIONS:
    --manifest-path <path>     Use voltex.toml from <path> instead of cwd
    --target <triple>          Build target triple. Must equal the host:
                                   windows-x64 | linux-x64 | macos-x64 | macos-arm64
                               (voltex build does not cross-compile.)
    --engine-path <repo>       Path to the voltex engine repo root.
                               Falls back to the VOLTEX_REPO environment
                               variable, then to the cwd's grandparent.
    --no-validate-paths        Skip the check that voltex.toml's scene /
                               scripts / assets paths exist. Useful for
                               build flows that synthesise paths.
    -h, --help                 Print this help

OUTPUTS (written to dist/ next to voltex.toml):
    dist/<name>.exe (Windows) or dist/<name> (Linux/macOS)
    dist/<name>.vscn                    Scene from manifest
    dist/scripts/                       Lua scripts (per glob)
    dist/assets/                        Assets (per glob)
    dist/THIRD-PARTY-LICENSES.md        License attribution (v1.0.12+)
    dist/LICENSE-MIT, LICENSE-APACHE    Engine license

EXAMPLES:
    # cwd is a voltex project (has voltex.toml)
    voltex build

    # Explicit engine path
    voltex build --engine-path /home/me/code/game_engine

    # CI / shell-env style
    VOLTEX_REPO=/path/to/engine voltex build
";

/// `voltex run --help` output.
pub const RUN: &str = "\
voltex run — build then launch

USAGE:
    voltex run [options]

OPTIONS:
    Same as `voltex build`. The resulting executable is spawned with
    its standard streams inherited from the current terminal, and its
    exit code becomes the exit code of `voltex run`.

EXAMPLES:
    voltex run
    voltex run --engine-path /path/to/game_engine
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_help_lists_all_commands() {
        assert!(TOP.contains("voltex new"));
        assert!(TOP.contains("voltex build"));
        assert!(TOP.contains("voltex run"));
        assert!(TOP.contains("--version"));
        assert!(TOP.contains("--help"));
    }

    #[test]
    fn build_help_documents_no_validate_paths() {
        // Surface for the v1.0.11 --no-validate-paths flag must reach the
        // user. Without this assertion the flag was silently undocumented.
        assert!(BUILD.contains("--no-validate-paths"));
    }

    #[test]
    fn build_help_documents_license_bundle_outputs() {
        // v1.0.12 promise: license files appear in dist/. If the OUTPUTS
        // section drops the line, users may not realise the artifact
        // already carries license attribution.
        assert!(BUILD.contains("THIRD-PARTY-LICENSES.md"));
        assert!(BUILD.contains("LICENSE-MIT"));
    }

    #[test]
    fn build_help_lists_engine_path_resolution_order() {
        assert!(BUILD.contains("--engine-path"));
        assert!(BUILD.contains("VOLTEX_REPO"));
    }

    #[test]
    fn new_help_describes_scaffold_contents() {
        assert!(NEW.contains("voltex.toml"));
        assert!(NEW.contains("scene.vscn"));
        assert!(NEW.contains("scripts/spinner.lua"));
    }

    #[test]
    fn run_help_references_build_options() {
        assert!(RUN.contains("Same as `voltex build`"));
    }

    #[test]
    fn top_help_includes_examples() {
        assert!(TOP.contains("EXAMPLES:"));
        assert!(TOP.contains("voltex new my_game"));
    }
}
