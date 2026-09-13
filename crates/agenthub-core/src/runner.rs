//! MCP 连通性测试（P1 第 7 项，差异化功能）：
//! - stdio：实际拉起进程，走 NDJSON JSON-RPC `initialize` 握手，验证后立即结束进程
//! - http/sse：健康检查（GET / POST initialize）
//!
//! 安全性：仅显式调用；子进程带超时，测试完必杀；不落盘、不改配置。

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::model::McpServerDef;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectivityResult {
    /// ok | failed
    pub status: String,
    pub latency_ms: u64,
    /// 握手成功时返回的 serverInfo.name
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    pub error: Option<String>,
}

fn ok_result(latency: u64, name: Option<String>, version: Option<String>) -> ConnectivityResult {
    ConnectivityResult {
        status: "ok".into(),
        latency_ms: latency,
        server_name: name,
        server_version: version,
        error: None,
    }
}

fn err_result(latency: u64, msg: String) -> ConnectivityResult {
    ConnectivityResult {
        status: "failed".into(),
        latency_ms: latency,
        server_name: None,
        server_version: None,
        error: Some(msg),
    }
}

fn initialize_request(id: u64) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"initialize\",\"params\":{{\"protocolVersion\":\"{PROTOCOL_VERSION}\",\"capabilities\":{{}},\"clientInfo\":{{\"name\":\"AgentHub\",\"version\":\"0.1.0\"}}}}}}\n"
    )
}

/// 测试任意定义（编辑抽屉保存前也可用）
pub fn test_def(def: &McpServerDef) -> ConnectivityResult {
    let started = Instant::now();
    if let Some(url) = &def.url {
        test_http(url, started)
    } else if let Some(cmd) = &def.command {
        test_stdio(cmd, &def.args, def.env.as_ref(), started)
    } else {
        err_result(0, "定义里既无 command 也无 url".into())
    }
}

/// 测试某个 Agent 配置里的条目（scope 目前只支持 global）
pub fn test_entry(reg: &crate::registry::Registry, agent_id: &str, name: &str, scope: &str) -> ConnectivityResult {
    let entry = match reg
        .all_mcp()
        .into_iter()
        .find(|e| e.agent_id == agent_id && e.name == name && e.scope == scope)
    {
        Some(e) => e,
        None => return err_result(0, format!("未找到条目 {name} @ {agent_id}")),
    };
    let def: McpServerDef = match serde_json::from_value(entry.raw) {
        Ok(d) => d,
        Err(e) => return err_result(0, format!("定义解析失败: {e}")),
    };
    test_def(&def)
}

/* ---------------- stdio ---------------- */

fn test_stdio(
    command: &str,
    args: &[String],
    env: Option<&serde_json::Map<String, serde_json::Value>>,
    started: Instant,
) -> ConnectivityResult {
    // Windows 上 npx/uvx 等是 .cmd，CreateProcess 不能直接执行，需要 cmd /C 包一层
    let needs_shell = cfg!(windows)
        && !Path::new(command).extension().is_some_and(|e| {
            e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("com")
        });
    let mut cmd = if needs_shell {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        for a in args {
            c.arg(a);
        }
        c
    } else {
        let mut c = Command::new(command);
        c.args(args);
        c
    };
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // 隐藏控制台窗口（Windows）
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    if let Some(map) = env {
        for (k, v) in map {
            if let Some(s) = v.as_str() {
                cmd.env(k, s);
            }
        }
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return err_result(
                started.elapsed().as_millis() as u64,
                format!("无法启动 {command}: {e}"),
            )
        }
    };

    let result = handshake(&mut child, started);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn handshake(child: &mut std::process::Child, started: Instant) -> ConnectivityResult {
    let Some(stdin) = child.stdin.take() else {
        return err_result(0, "无法取得子进程 stdin".into());
    };
    let Some(stdout) = child.stdout.take() else {
        return err_result(0, "无法取得子进程 stdout".into());
    };
    let mut stdin = stdin;

    if let Err(e) = stdin.write_all(initialize_request(1).as_bytes()) {
        return err_result(elapsed(started), format!("写入握手请求失败: {e}"));
    }
    let _ = stdin.flush();
    // 握手完成后关闭 stdin，通知规范实现可以退出
    drop(stdin);

    // 关键：读必须放在独立线程。read_line 是阻塞调用，进程不输出时
    // 会永远挂起——主循环用 recv_timeout 才能实现真正的超时。
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = tx.send(None);
                    break;
                }
                Ok(_) => {
                    if tx.send(Some(line)).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = tx.send(None);
                    break;
                }
            }
        }
    });

    let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return err_result(elapsed(started), "握手超时（15s 无响应）".into());
        }
        match rx.recv_timeout(remaining) {
            Ok(None) => {
                return err_result(
                    elapsed(started),
                    "进程提前退出（命令不存在、参数错误或启动即崩溃）".into(),
                );
            }
            Ok(Some(line)) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // 容忍 LSP 风格 Content-Length 头（个别实现）
                if trimmed.starts_with("Content-Length:") {
                    continue;
                }
                match serde_json::from_str::<serde_json::Value>(trimmed) {
                    Ok(v) if v.get("id") == Some(&serde_json::json!(1)) => {
                        if let Some(err) = v.get("error") {
                            return err_result(elapsed(started), format!("服务端返回错误: {err}"));
                        }
                        let info = v.pointer("/result/serverInfo");
                        return ok_result(
                            elapsed(started),
                            info.and_then(|i| i.get("name")).and_then(|n| n.as_str()).map(String::from),
                            info.and_then(|i| i.get("version")).and_then(|n| n.as_str()).map(String::from),
                        );
                    }
                    Ok(_) => continue,   // 通知或其他消息
                    Err(_) => continue,  // 非 JSON 行（启动横幅等）
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                return err_result(elapsed(started), "握手超时（15s 无响应）".into());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return err_result(elapsed(started), "输出流异常关闭".into());
            }
        }
    }
}

fn elapsed(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

/* ---------------- http ---------------- */

fn test_http(url: &str, started: Instant) -> ConnectivityResult {
    use crate::market;
    // 优先 POST initialize（streamable http），失败退化 GET 健康检查
    let body = initialize_request(1).trim_end().to_string();
    match market::post_json(url, &body) {
        Ok(v) => {
            let info = v.pointer("/result/serverInfo");
            ok_result(
                elapsed(started),
                info.and_then(|i| i.get("name")).and_then(|n| n.as_str()).map(String::from),
                info.and_then(|i| i.get("version")).and_then(|n| n.as_str()).map(String::from),
            )
        }
        Err(e) => {
            // 退化：GET 能连通也算“服务在线但非 MCP 端点”
            match market::get_status(url) {
                Ok(code) => err_result(
                    elapsed(started),
                    format!("HTTP {code} 在线，但 initialize 握手失败: {e}"),
                ),
                Err(_) => err_result(elapsed(started), e.to_string()),
            }
        }
    }
}
