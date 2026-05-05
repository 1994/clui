use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=MIHOMO_BIN");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let out_core = out_dir.join(if cfg!(windows) {
        "mihomo.exe"
    } else {
        "mihomo"
    });

    let embedded_enabled = env::var_os("CARGO_FEATURE_EMBEDDED_CORE").is_some();
    let source = env::var_os("MIHOMO_BIN").map(PathBuf::from);

    if embedded_enabled
        && let Some(source) = source
        && source.is_file()
    {
        println!("cargo:rerun-if-changed={}", source.display());
        fs::copy(&source, &out_core).expect("copy MIHOMO_BIN into build output");
        return;
    }

    fs::write(out_core, []).expect("write empty embedded core placeholder");
}
