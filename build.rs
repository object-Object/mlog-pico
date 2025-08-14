use std::{env, fs, path::PathBuf};

use eg_font_converter::FontConverter;
use glob::glob;
use mindy::parser::{LogicParser, parse_and_serialize_ast};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    // pre-parse mlog files

    let mlog_dir = out_dir.join("mlog");
    fs::create_dir(&mlog_dir).ok(); // ignore error if directory already exists

    let parser = LogicParser::new();
    for path in glob("src/**/*.mlog").unwrap().flatten() {
        println!("cargo:rerun-if-changed={}", path.display());

        let code = fs::read_to_string(&path).unwrap();
        let ast = parse_and_serialize_ast(&parser, &code, true).unwrap();

        let out = mlog_dir.join(path.with_extension("bin").file_name().unwrap());
        fs::write(&out, ast).unwrap();
    }

    // set up embassy memory.x

    println!("cargo:rerun-if-changed=memory-pico1.x");
    println!("cargo:rerun-if-changed=memory-pico2.x");

    #[cfg(feature = "pico1")]
    let memory_x = include_bytes!("memory-pico1.x");
    #[cfg(feature = "pico2")]
    let memory_x = include_bytes!("memory-pico2.x");

    fs::write(out_dir.join("memory.x"), memory_x).unwrap();

    println!("cargo:rustc-link-search={}", out_dir.display());

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");

    #[cfg(feature = "pico1")]
    println!("cargo:rustc-link-arg-bins=-Tlink-rp.x");

    // generate fonts

    println!("cargo:rerun-if-changed=fonts/mindustry/logic.bdf");

    // https://github.com/Anuken/Mindustry/blob/65a50a97423431640e636463dde97f6f88a2b0c8/core/src/mindustry/ui/Fonts.java#L88C27-L88C126
    FontConverter::with_file("fonts/mindustry/logic.bdf", "LOGIC")
        .glyphs("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz1234567890\"!`?'.,;:()[]{}<>|/@\\^$€-%+=#_&~* ")
        .replacement_character(' ')
        .convert_mono_font()
        .unwrap()
        .save(&out_dir)
        .unwrap();
}
