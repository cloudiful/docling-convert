use clap::Parser;
use cloudiful_docling_convert_web::{WebServerConfig, run_web_server};

#[derive(Debug, Parser)]
#[command(
    name = "cloudiful-docling-convert-web",
    version,
    about = "Run the cloudiful-docling-convert Axum web server"
)]
struct Args {
    #[arg(
        long,
        default_value = "http://127.0.0.1:5001/v1",
        value_name = "URL",
        help = "Docling API base URL"
    )]
    docling_base_url: String,

    #[arg(
        long,
        default_value = "https://api.openai.com/v1",
        value_name = "URL",
        help = "OpenAI-compatible API base URL for VLM"
    )]
    openai_base_url: String,

    #[arg(
        long,
        default_value = "gpt-4o-mini",
        value_name = "MODEL",
        help = "VLM Pipeline model"
    )]
    vlm_pipeline_model: String,

    #[arg(
        long,
        default_value = "gpt-4o-mini",
        value_name = "MODEL",
        help = "VLM model for picture descriptions"
    )]
    picture_description_model: String,

    #[arg(
        long,
        default_value = "gpt-4o-mini",
        value_name = "MODEL",
        help = "VLM model for code and formula recognition"
    )]
    code_formula_model: String,

    #[arg(
        long,
        default_value = "localhost",
        value_name = "HOST",
        help = "Web server host (default: localhost)"
    )]
    host: String,

    #[arg(
        short,
        long,
        default_value_t = 3000,
        value_name = "PORT",
        help = "Web server port (default: 3000)"
    )]
    port: u16,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let args = Args::parse();

    if let Err(error) = run_web_server(WebServerConfig {
        docling_base_url: args.docling_base_url,
        openai_base_url: args.openai_base_url,
        vlm_pipeline_model: args.vlm_pipeline_model,
        picture_description_model: args.picture_description_model,
        code_formula_model: args.code_formula_model,
        host: args.host,
        port: args.port,
    })
    .await
    {
        eprintln!("Web server failed: {}", error);
        std::process::exit(1);
    }
}
