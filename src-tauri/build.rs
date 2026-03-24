#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
include!("scoring_dsl.rs");

fn main() {
    tauri_build::build();

    // Compile scoring DSL → scoring_config.rs
    println!("cargo:rerun-if-changed=scoring/pipeline.scoring");

    let dsl_path = std::path::Path::new("scoring/pipeline.scoring");
    if dsl_path.exists() {
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
        let output_path = std::path::Path::new(&out_dir).join("scoring_config.rs");

        let input =
            std::fs::read_to_string(dsl_path).expect("Failed to read scoring/pipeline.scoring");

        if let Err(errors) = compile_scoring_dsl(&input, &output_path) {
            for err in &errors {
                println!("cargo:warning=Scoring DSL error: {}", err);
            }
            panic!(
                "Scoring DSL compilation failed with {} error(s)",
                errors.len()
            );
        }
    }
}
