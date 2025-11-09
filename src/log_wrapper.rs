pub mod log_wrapper {
    use time::macros::format_description;
    use tracing_appender::rolling;
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

    /// Initialize log
    pub fn init() -> tracing_appender::non_blocking::WorkerGuard {
        let file_appender = rolling::daily("./logs", "app.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let time_format = format_description!(
            "[year]-[month padding:zero]-[day padding:zero] [hour padding:zero]:[minute padding:zero]:[second padding:zero].[subsecond digits:3]"
        );

        let local_offset = time::UtcOffset::local_offset_at(
            time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc()),
        )
        .unwrap_or(time::UtcOffset::UTC);

        // 统一的日志格式配置
        let log_format = fmt::format::Format::default()
            .compact()
            .with_timer(fmt::time::OffsetTime::new(local_offset, time_format))
            .with_thread_names(true)
            .with_thread_ids(false)
            .with_level(true)
            .with_target(false)
            .with_file(false)
            .with_line_number(false);

        // 控制台输出层
        let console_layer = fmt::layer()
            .event_format(log_format.clone())
            .with_writer(std::io::stdout)
            .with_ansi(true);

        // 文件输出层
        let file_layer = fmt::layer()
            .event_format(log_format)
            .with_ansi(false)
            .with_writer(non_blocking);

        // 注册并初始化
        tracing_subscriber::registry()
            .with(console_layer)
            .with(file_layer)
            .init();

        guard
    }
}
