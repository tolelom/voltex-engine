//! Build script — compiles the bundled Lua 5.4 C sources into a static
//! library that `voltex_script` links against. Re-runs only when `lua/`
//! sources change.

fn main() {
    let mut build = cc::Build::new();
    build.include("lua");

    // Add all .c files except lua.c and luac.c (standalone executables with main())
    for entry in std::fs::read_dir("lua").unwrap() {
        let path = entry.unwrap().path();
        if let Some(ext) = path.extension() {
            if ext == "c" {
                let name = path.file_name().unwrap().to_str().unwrap();
                if name != "lua.c" && name != "luac.c" {
                    build.file(&path);
                }
            }
        }
    }

    build.compile("lua54");
}
