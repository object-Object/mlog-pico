use std::{
    env, fs,
    path::{MAIN_SEPARATOR, Path},
};

use glob::glob;
use mindustry_rs::logic::{LogicParser, parse_and_serialize_ast};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let parser = LogicParser::new();
    let out_dir = env::var_os("OUT_DIR").unwrap();

    for path in glob("**/*.mlog").unwrap().flatten() {
        println!("cargo:rerun-if-changed={}", path.to_string_lossy());

        let code = fs::read_to_string(&path).unwrap();
        let ast = parse_and_serialize_ast(&parser, &code, true).unwrap();

        let out = path.with_extension("bin");
        let out = Path::new(&out_dir).join(out.file_name().unwrap());
        fs::write(&out, ast).unwrap();

        println!(
            "cargo:rustc-env=MLOG:{}={}",
            path.to_string_lossy().replace(MAIN_SEPARATOR, "/"),
            out.to_string_lossy()
        )
    }
}
