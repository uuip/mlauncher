use anyhow::Context;
use crossbeam_channel::{unbounded, Sender};
use log::{error, info, log, warn, Level};
use regex::Regex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, LazyLock, Mutex};
use std::{env, thread};

use crate::logger::init_logger;

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
    let child = Command::new("./mihomo-darwin-arm64")
        .current_dir(&cwd)
        .arg("-d")
        .arg(".")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("无法启动 mihomo (当前目录: {})", cwd.display()))?;

    let child_arc = Arc::new(Mutex::new(child));

    let child_for_signal = child_arc.clone();
    ctrlc::set_handler(move || {
        let mut child_guard = child_for_signal.lock().unwrap();
        match child_guard.try_wait() {
            Ok(Some(status)) => info!("子进程已退出，状态: {}", status),
            Ok(None) => {
                info!("终止子进程...");
                if let Err(e) = child_guard.kill() {
                    error!("终止子进程失败: {}", e);
                }
            }
            Err(e) => error!("error attempting to wait: {e}"),
        }
    })?;

    let (sender, receiver) = unbounded();
    let child_lock = child_arc.clone();
    thread::spawn(move || handle_output(child_lock, sender));

    thread::spawn(move || {
        for line in receiver {
            if line.contains("[TUN] Tun adapter listening at: utun") {
                thread::spawn(|| {
                    warn!("检测到关键词，正在设置 DNS...");
                    set_dns("198.18.0.2");
                });
            }
            log(&line);
        }
    })
    .join()
    .unwrap();

    Ok(())
}

fn handle_output(child_arc: Arc<Mutex<Child>>, sender: Sender<String>) {
    let mut child = child_arc.lock().unwrap();
    let stdout = child.stdout.take().expect("Failed to get stdout");
    let stderr = child.stderr.take().expect("Failed to get stderr");
    let sender_stdout = sender.clone();
    let sender_stderr = sender;
    let handle_stdout = thread::spawn(move || read_pipe(stdout, sender_stdout));
    let handle_stderr = thread::spawn(move || read_pipe(stderr, sender_stderr));

    handle_stdout.join().unwrap();
    handle_stderr.join().unwrap();
}

fn read_pipe<R: std::io::Read>(pipe: R, sender: Sender<String>) {
    let reader = BufReader::new(pipe);
    for line in reader.lines().map_while(Result::ok) {
        if sender.send(line).is_err() {
            break;
        }
    }
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
        let time = format!("{}.{}{}", &caps[1], &caps[2], &caps[3]);
        let level = &caps[4];
        let level = *LOG_LEVEL.get(level).unwrap_or(&Level::Info);
        let msg = &caps[5];

        log!(level, "{} {}", time, msg);
    }
}
