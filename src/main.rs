use crate::logger::{init_logger, log_mihomo_line};
use anyhow::Context;
use duct::cmd;
use log::{error, info};
use std::env;
use std::io::{BufRead, BufReader};
use std::process::Command;
use std::sync::Arc;
mod logger;

struct DnsGuard;
impl Drop for DnsGuard {
    fn drop(&mut self) {
        info!("Restoring DNS...");
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
        .with_context(|| {
            format!(
                "Failed to start mihomo (current directory: {})",
                cwd.display()
            )
        })?;

    let child_arc = Arc::new(child);

    let child_guard = child_arc.clone();
    ctrlc::set_handler(move || match child_guard.try_wait() {
        Ok(Some(_)) => info!("Child process has exited"),
        Ok(None) => {
            info!("Terminating child process...");
            if let Err(e) = child_guard.kill() {
                error!("Failed to terminate child process: {e}");
            }
        }
        Err(e) => error!("error attempting to wait: {e}"),
    })?;

    let lines = BufReader::new(child_arc.as_ref()).lines();
    for line in lines.map_while(Result::ok) {
        log_mihomo_line(&line);
        if line.contains("[TUN] default interface changed by monitor") {
            info!("Keyword detected, setting DNS...");
            set_dns("198.18.0.2");
        }
    }

    Ok(())
}

fn set_dns(dns: &str) {
    if let Some(interface) = get_friendly_name() {
        let status = Command::new("networksetup")
            .args(["-setdnsservers", &interface, dns])
            .status();
        if let Err(e) = status {
            error!("Failed to set DNS: {e}");
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
