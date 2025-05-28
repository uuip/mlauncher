use log::LevelFilter;
use std::io::Write;

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
