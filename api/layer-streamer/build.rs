use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let shader_dir = PathBuf::from(&manifest_dir).join("src").join("shaders");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let shader_out_dir = out_dir.join("shaders");
    std::fs::create_dir_all(&shader_out_dir).unwrap();

    let shaders = &["matmul.comp", "rms_norm.comp", "activation.comp"];

    for shader in shaders {
        let input = shader_dir.join(shader);
        let output_name = shader.replace(".comp", ".spv");
        let output = shader_out_dir.join(&output_name);

        println!("cargo::rerun-if-changed={}", input.display());

        let status = Command::new("glslc")
            .arg("--target-env=vulkan1.2")
            .arg("-o")
            .arg(&output)
            .arg(&input)
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo::warning=Compiled shader: {} -> {}", shader, output_name);
            }
            _ => {
                // glslc not available, check for pre-compiled shaders
                if !output.exists() {
                    println!(
                        "cargo::warning=glslc not found, shaders must be pre-compiled: {}",
                        shader
                    );
                    // Create empty placeholder
                    std::fs::write(&output, &[]).unwrap();
                }
            }
        }
    }
}
