use env_logger::fmt::style::Color;
use log::{Level, LevelFilter};
use std::io::Write;

pub fn init_logger() {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .format(|buf, record| {
            let color = match record.level() {
                Level::Warn => Some(Color::Ansi256(215_u8.into())),
                Level::Error => Some(Color::Ansi256(203_u8.into())),
                _ => None,
            };

            let level_style = buf.default_level_style(record.level());
            let reset = level_style.render_reset();
            let render = level_style.fg_color(color).render();
            writeln!(
                buf,
                "{render}[{}]: {}{reset}",
                record.level(),
                record.args()
            )
        })
        .init();
}
