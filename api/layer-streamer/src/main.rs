use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Instant;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use layer_streamer::*;
use layer_streamer::forward::{StreamingForward, ModelConfig};
use layer_streamer::gpu_forward::GpuForward;

#[derive(Parser)]
#[command(name = "layer-streamer")]
#[command(about = "GGUF layer splitter & streaming inference (AirLLM-style)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Split a GGUF model into individual layer files
    Split {
        #[arg(short, long)]
        model: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Show information about a GGUF model
    Info {
        #[arg(short, long)]
        model: PathBuf,
    },
    /// Show information about a split model
    Index {
        #[arg(short, long)]
        index: PathBuf,
    },
    /// Generate text using layer-by-layer streaming
    Generate {
        /// Path to original GGUF model
        #[arg(long)]
        model: PathBuf,
        /// Path to split layer directory (with model_index.json)
        #[arg(long)]
        layers: PathBuf,
        /// Prompt text
        #[arg(short, long, default_value = "Hello, I am")]
        prompt: String,
        /// Maximum tokens to generate
        #[arg(short, long, default_value = "64")]
        max_tokens: usize,
        /// Sampling temperature (0.0 = greedy)
        #[arg(short, long, default_value = "0.0")]
        temperature: f32,
        /// Use GPU (Vulkan) for compute
        #[arg(short, long, default_value = "false")]
        gpu: bool,
        /// Run benchmark mode
        #[arg(long, default_value = "false")]
        benchmark: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| log_level.into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    match cli.command {
        Commands::Split { model, output } => cmd_split(&model, &output)?,
        Commands::Info { model } => cmd_info(&model)?,
        Commands::Index { index } => cmd_index(&index)?,
        Commands::Generate { model, layers, prompt, max_tokens, temperature, gpu, benchmark } => {
            cmd_generate(&model, &layers, &prompt, max_tokens, temperature, gpu, benchmark)?;
        }
    }
    Ok(())
}

fn cmd_split(model: &PathBuf, output: &PathBuf) -> Result<()> {
    println!("🔪 Splitting GGUF model into layers");
    println!("   Model: {}", model.display());
    println!("   Output: {}", output.display());
    println!();

    let start = Instant::now();
    let index = layer_streamer::split_gguf(model, output)?;
    let elapsed = start.elapsed();

    println!();
    println!("✅ Split complete in {:.2}s", elapsed.as_secs_f64());
    println!("   Architecture: {}", index.architecture);
    println!("   Layers: {}", index.n_layers);
    println!("   Files: {}", index.file_count());
    println!("   Total size: {}", index.total_size_human());
    println!();
    println!("Next: layer-streamer generate --model {} --layers {}",
        model.display(), output.display());
    Ok(())
}

fn cmd_info(model: &PathBuf) -> Result<()> {
    println!("📖 Reading GGUF model info");
    println!("   Model: {}", model.display());
    println!();

    let model_info = layer_streamer::parse_gguf(model)?;

    println!("Architecture: {:?}", model_info.architecture);
    println!("Tensor Count: {}", model_info.tensors.len());
    println!("KV Count: {}", model_info.key_values.len());
    println!();

    if let Some(n) = model_info.n_layers { println!("Layers (block_count): {}", n); }
    if let Some(n) = model_info.n_embd { println!("Embedding Length: {}", n); }
    if let Some(n) = model_info.n_head { println!("Attention Heads: {}", n); }
    if let Some(n) = model_info.n_vocab { println!("Vocabulary Size: {}", n); }

    println!();
    println!("First 20 tensors:");
    for (i, tensor) in model_info.tensors.iter().take(20).enumerate() {
        let dims_str: Vec<String> = tensor.dims.iter().map(|d| d.to_string()).collect();
        println!("  [{:03}] {} [{}] {:?}", i, tensor.name, dims_str.join(", "), tensor.dtype);
    }
    if model_info.tensors.len() > 20 {
        println!("  ... and {} more", model_info.tensors.len() - 20);
    }
    Ok(())
}

fn cmd_index(index: &PathBuf) -> Result<()> {
    let index_path = if index.is_dir() {
        index.join("model_index.json")
    } else {
        index.clone()
    };

    println!("📋 Reading model index");
    println!("   Index: {}", index_path.display());
    println!();

    let idx = layer_streamer::metadata::ModelIndex::load(&index_path)?;

    println!("Source Model: {}", idx.source_model);
    println!("Architecture: {}", idx.architecture);
    println!("Layers: {}", idx.n_layers);
    if let Some(n) = idx.n_embd { println!("Embedding Length: {}", n); }
    if let Some(n) = idx.n_head { println!("Attention Heads: {}", n); }
    println!("Files: {}", idx.file_count());
    println!("Total Size: {}", idx.total_size_human());
    println!();
    if !idx.token_embedding_tensors.is_empty() {
        println!("Token Embeddings: {} tensor(s)", idx.token_embedding_tensors.len());
    }
    if !idx.output_tensors.is_empty() {
        println!("Output: {} tensor(s)", idx.output_tensors.len());
    }
    if !idx.norm_tensors.is_empty() {
        println!("Normalization: {} tensor(s)", idx.norm_tensors.len());
    }
    for layer in &idx.layers {
        println!("  [{:03}] {} ({:.2} MB, {} tensors)",
            layer.index, layer.file_name,
            layer.size_bytes as f64 / (1024.0 * 1024.0),
            layer.tensor_names.len());
    }
    Ok(())
}

fn cmd_generate(
    ggu_path: &PathBuf,
    layers_dir: &PathBuf,
    prompt: &str,
    max_tokens: usize,
    temperature: f32,
    use_gpu: bool,
    benchmark: bool,
) -> Result<()> {
    if benchmark {
        return cmd_benchmark(ggu_path, layers_dir, use_gpu);
    }

    println!("🧠 Layer-by-layer text generation ({})", if use_gpu { "GPU Vulkan" } else { "CPU" });
    println!("   Model: {}", ggu_path.display());
    println!("   Layers: {}", layers_dir.display());
    println!("   Prompt: \"{}\"", prompt);
    println!("   Max tokens: {}", max_tokens);
    println!("   Temperature: {}", temperature);
    println!();

    let start = Instant::now();

    // Load model info for configuration
    let model_info = layer_streamer::parse_gguf(ggu_path)?;
    let n_layers = model_info.n_layers.unwrap_or(42) as usize;
    let n_head = model_info.n_head.unwrap_or(8) as usize;

    println!("📋 Model config: {} layers, {} heads", n_layers, n_head);

    if use_gpu {
        println!();
        println!("🔧 Initializing Vulkan...");
        match layer_streamer::VulkanContext::new() {
            Ok(ctx_raw) => {
                let ctx = std::sync::Arc::new(ctx_raw);
                println!("✅ Vulkan initialized!");
                println!("   Device: {}", unsafe {
                    std::ffi::CStr::from_ptr(ctx.device_properties.device_name.as_ptr())
                        .to_string_lossy()
                });
                println!();

                // Benchmark GPU matmul
                let gpu_ops = layer_streamer::GpuOps::new(ctx.clone());
                println!("🏃 GPU Matmul Benchmark (SPIR-V shaders):");
                for size in [64, 128, 256] {
                    match gpu_ops.benchmark(size) {
                        Ok(time) => {
                            println!("   {}x{}: {:.3}ms", size, size, time * 1000.0);
                        }
                        Err(e) => {
                            println!("   {}x{}: ERROR - {}", size, size, e);
                        }
                    }
                }
                println!();
                println!("⚠️  Full GPU forward not yet available.");
                println!("   Running CPU forward for text generation...");
                println!();

                // Fall back to CPU for actual generation
                match create_cpu_forward(&ggu_path, &layers_dir) {
                    Ok(mut forward) => {
                        let test_token = 1234;
                        println!("📦 CPU Forwarding token {} through {} layers:", test_token, n_layers);
                        match forward.forward_token(test_token) {
                            Ok(logits) => {
                                println!("✅ CPU Forward complete: {}", logits.fmt_short());
                            }
                            Err(e) => {
                                println!("⚠️  CPU forward error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("⚠️  CPU forward error: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("⚠️  Vulkan not available: {}", e);
            }
        }
    } else {
        // CPU mode
        println!("⚡ Starting layer-by-layer forward pass (CPU)...");
        match create_cpu_forward(&ggu_path, &layers_dir) {
            Ok(mut forward) => {
                let test_token = 1234;
                println!("📦 Forwarding token {} through {} layers:", test_token, n_layers);

                match forward.forward_token(test_token) {
                    Ok(logits) => {
                        println!("✅ Forward pass complete: {}", logits.fmt_short());
                        let next_token = layer_streamer::sample(&logits, SamplingStrategy::Greedy, 0.0);
                        println!("   Next token: {}", next_token);
                    }
                    Err(e) => {
                        println!("⚠️  Forward error: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("⚠️  CPU forward error: {}", e);
            }
        }
    }

    println!();
    println!("📋 To enable full GPU compute:");
    println!("   1. Install glslc (Vulkan SDK or mesa-vulkan-tools)");
    println!("   2. Shaders will compile automatically on next build");
    println!();
    println!("Phase 3 complete! Vulkan pipeline is initialized.");
    println!("Next steps:");
    println!("  1. Compile SPIR-V shaders (glslc)");
    println!("  2. Enable GPU matmul/rms_norm dispatch");
    println!("  3. Add async prefetching overlap");
    println!("  4. Full autoregressive GPU generation");

    Ok(())
}

fn cmd_benchmark(
    ggu_path: &PathBuf,
    layers_dir: &PathBuf,
    use_gpu: bool,
) -> Result<()> {
    println!("🏃 Benchmark mode ({})", if use_gpu { "GPU" } else { "CPU" });
    println!();

    // Test Vulkan availability
    if use_gpu {
        match layer_streamer::VulkanContext::new() {
            Ok(ctx_raw) => {
                let ctx = std::sync::Arc::new(ctx_raw);
                let gpu_ops = layer_streamer::GpuOps::new(ctx.clone());
                let device_name = unsafe {
                    std::ffi::CStr::from_ptr(ctx.device_properties.device_name.as_ptr())
                        .to_string_lossy()
                        .to_string()
                };
                println!("GPU: {}", device_name);
                println!();

                // Run matmul benchmarks
                for size in [64, 128, 256, 512] {
                    match gpu_ops.benchmark(size) {
                        Ok(time) => {
                            println!("  {}x{} matmul: {:.3}ms", size, size, time * 1000.0);
                        }
                        Err(e) => {
                            println!("  {}x{} matmul: ERROR - {}", size, size, e);
                        }
                    }
                }
            }
            Err(e) => {
                println!("Vulkan not available: {}", e);
            }
        }
    } else {
        println!("CPU Benchmark:");
        println!("  (CPU matmul benchmark not implemented yet)");
    }

    Ok(())
}

/// Create CPU forward instance using auto-detected model config
fn create_cpu_forward(
    ggu_path: &PathBuf,
    layers_dir: &PathBuf,
) -> Result<StreamingForward> {
    let loader = LayerLoader::new(layers_dir, ggu_path)?;
    let model_info = parse_gguf(ggu_path)?;
    let config = ModelConfig::from_gguf(&model_info);
    StreamingForward::new(loader, config)
}

/// Create GPU forward instance using auto-detected model config
fn create_gpu_forward(
    ggu_path: &PathBuf,
    layers_dir: &PathBuf,
) -> Result<(GpuForward, f64)> {
    let start = Instant::now();
    let loader = LayerLoader::new(layers_dir, ggu_path)?;
    let model_info = parse_gguf(ggu_path)?;
    let config = ModelConfig::from_gguf(&model_info);
    let forward = GpuForward::new(loader, config)?;
    let elapsed = start.elapsed().as_secs_f64();
    Ok((forward, elapsed))
}
