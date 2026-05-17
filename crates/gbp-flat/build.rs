fn main() {
    let fbs_files = ["fbs/gbp.fbs", "fbs/gtp.fbs", "fbs/gap.fbs", "fbs/gsp.fbs"];

    for f in &fbs_files {
        println!("cargo:rerun-if-changed={f}");
    }

    let declarations = planus_translation::translate_files(&fbs_files)
        .expect("FlatBuffers schema parse/validation failed");

    let code = planus_codegen::generate_rust(&declarations, false)
        .expect("FlatBuffers Rust code generation failed");

    let out = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{out}/generated.rs"), code).unwrap();
}
