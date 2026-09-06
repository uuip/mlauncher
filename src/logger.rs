use log::{Level, LevelFilter, log};
use regex::Regex;
use std::io::Write;
use std::sync::LazyLock;

static RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"time="(.*?)\.(\d{3})\d*([+-]\d{2}:\d{2})"\s+level=(\w+)\s+msg="(.*?)""#).unwrap()
});

pub fn init_logger() {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .format(|buf, record| {
            let level_style = buf.default_level_style(record.level());
            let reset = level_style.render_reset();
            let render = level_style.render();
            writeln!(
                buf,
                "{render}[{}]: {}{reset}",
                record.level(),
                record.args()
            )
        })
        .init();
}

pub fn log_mihomo_line(s: &str) {
    if let Some(caps) = RE.captures(s) {
        let level = match &caps[4] {
            "debug" => Level::Debug,
            "warning" => Level::Warn,
            "error" => Level::Error,
            _ => Level::Info,
        };
        let msg = &caps[5];

        log!(target: env!("CARGO_CRATE_NAME"), level, "{}.{}{} {}", &caps[1], &caps[2], &caps[3], msg);
    }
}
