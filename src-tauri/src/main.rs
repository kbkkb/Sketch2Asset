#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    sketch2asset_lib::matting::preload_startup_runtime_dlls();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(first) = args.first() {
        if first == "--mcp" || first == "cli" {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to initialize tokio runtime");
            rt.block_on(async {
                if first == "cli" {
                    let _ = sketch2asset_lib::mcp::run_cli_args(&args[1..]).await;
                } else {
                    let _ = sketch2asset_lib::mcp::run_mcp_stdio_loop().await;
                }
            });
            return;
        }
    }
    sketch2asset_lib::run()
}
