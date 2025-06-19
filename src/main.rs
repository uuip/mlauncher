use crate::logger::init_logger;
use anyhow::Context;
use duct::cmd;
use log::{Level, error, info, log, warn};
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader};
use std::process::Command;
use std::sync::{Arc, LazyLock};
mod logger;

static LOG_LEVEL: LazyLock<HashMap<&'static str, Level>> = LazyLock::new(|| {
    let mapper: [(&str, Level); 4] = [
        ("debug", Level::Debug),
        ("info", Level::Info),
        ("warning", Level::Warn),
        ("error", Level::Error),
    ];
    HashMap::from(mapper)
});
static RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"time="(.*?)\.(\d{3})\d*([+-]\d{2}:\d{2})"\s+level=(\w+)\s+msg="(.*?)""#).unwrap()
});

struct DnsGuard;
impl Drop for DnsGuard {
    fn drop(&mut self) {
        set_dns("empty");
    }
}

fn main() -> anyhow::Result<()> {
    init_logger();
    let _dns_guard = DnsGuard;
    let cwd = env::current_dir().context("Failed to get current directory")?;
    let child = cmd!("./mihomo-darwin-arm64", "-d", ".")
        .dir(&cwd)
        .stderr_to_stdout()
        .reader()
        .with_context(|| format!("无法启动 mihomo (当前目录: {})", cwd.display()))?;

    let child_arc = Arc::new(child);

    let child_guard = child_arc.clone();
    ctrlc::set_handler(move || match child_guard.try_wait() {
        Ok(Some(_)) => info!("子进程已退出，状态"),
        Ok(None) => {
            info!("终止子进程...");
            if let Err(e) = child_guard.kill() {
                error!("终止子进程失败: {}", e);
            }
        }
        Err(e) => error!("error attempting to wait: {e}"),
    })?;

    let reader = child_arc.clone();
    let lines = BufReader::new(&*reader).lines();
    for line in lines.map_while(Result::ok) {
        if line.contains("[TUN] Tun adapter listening at: utun") {
            warn!("检测到关键词，正在设置 DNS...");
            set_dns("198.18.0.2");
        }
        log(&line);
    }

    Ok(())
}

fn set_dns(dns: &str) {
    if let Some(interface) = get_friendly_name() {
        let status = Command::new("networksetup")
            .args(["-setdnsservers", &interface, dns])
            .status();
        if let Err(e) = status {
            error!("DNS 设置失败: {}", e);
        }
    }
}

fn get_friendly_name() -> Option<String> {
    let interfaces = netdev::get_interfaces();
    interfaces
        .into_iter()
        .find(|i| i.is_physical() && !i.ipv4.is_empty() && i.gateway.is_some())
        .and_then(|interface| interface.friendly_name)
}

fn log(s: &str) {
    if let Some(caps) = RE.captures(s) {
        let level = &caps[4];
        let level = *LOG_LEVEL.get(level).unwrap_or(&Level::Info);
        let msg = &caps[5];

        log!(level, "{}.{}{} {}", &caps[1], &caps[2], &caps[3], msg);
    }
}
