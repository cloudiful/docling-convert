use cloudiful_docling_convert::{PdfConvertError, Result};
use log::{LevelFilter, info};
use std::path::PathBuf;

pub fn init_logging() -> Result<()> {
    use log4rs::append::console::ConsoleAppender;
    use log4rs::append::rolling_file::RollingFileAppender;
    use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
    use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
    use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
    use log4rs::config::{Appender, Config, Logger, Root};
    use log4rs::encode::pattern::PatternEncoder;

    let log_dir = get_log_dir()?;

    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir)
            .map_err(|e| PdfConvertError::io_error("creating log directory", e))?;
    }

    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "[{d(%Y-%m-%d %H:%M:%S)} {h({l})} {M}:{L}] {m}{n}",
        )))
        .build();

    let log_file_path = log_dir.join("cloudiful-docling-convert.log");
    let file = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "[{d(%Y-%m-%d %H:%M:%S.%f)} {l} {M}:{L}] {m}{n}",
        )))
        .build(
            log_file_path.clone(),
            Box::new(CompoundPolicy::new(
                Box::new(SizeTrigger::new(10 * 1024 * 1024)),
                Box::new(
                    FixedWindowRoller::builder()
                        .base(1)
                        .build(
                            &format!(
                                "{}/cloudiful-docling-convert-{{}}.log.zst",
                                log_dir.display()
                            ),
                            100,
                        )
                        .map_err(|e| PdfConvertError::api_error(None, e.to_string()))?,
                ),
            )),
        )
        .map_err(|e| {
            PdfConvertError::io_error(
                "building file appender",
                std::io::Error::other(e.to_string()),
            )
        })?;

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("file", Box::new(file)))
        .logger(
            Logger::builder()
                .appender("stdout")
                .appender("file")
                .additive(false)
                .build("cloudiful_docling_convert", LevelFilter::Info),
        )
        .build(
            Root::builder()
                .appender("stdout")
                .appender("file")
                .build(LevelFilter::Info),
        )
        .map_err(|e| PdfConvertError::io_error("building log config", std::io::Error::other(e)))?;

    log4rs::init_config(config)
        .map_err(|e| PdfConvertError::io_error("initializing logger", std::io::Error::other(e)))?;

    info!("Logging initialized. Log file: {}", log_file_path.display());

    Ok(())
}

fn get_log_dir() -> Result<PathBuf> {
    if let Some(cache_dir) = dirs::cache_dir() {
        return Ok(cache_dir.join("cloudiful-docling-convert"));
    }

    std::env::current_dir()
        .map(|d| d.join("logs"))
        .map_err(|e| PdfConvertError::io_error("getting current directory", e))
}
