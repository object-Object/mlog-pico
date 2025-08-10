use std::{env, fs, path::Path};

use glob::glob;
use mindustry_rs::logic::{LogicParser, parse_and_serialize_ast};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir).join("mlog");
    fs::create_dir(&out_dir).ok(); // ignore error if directory already exists

    let parser = LogicParser::new();
    for path in glob("src/**/*.mlog").unwrap().flatten() {
        println!("cargo:rerun-if-changed={}", path.to_string_lossy());

        let code = fs::read_to_string(&path).unwrap();
        let ast = parse_and_serialize_ast(&parser, &code, true).unwrap();

        let out = out_dir.join(path.with_extension("bin").file_name().unwrap());
        fs::write(&out, ast).unwrap();
    }
}
