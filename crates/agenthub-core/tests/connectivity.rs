//! MCP 连通性测试：stdio 握手用 Python 假服务器做确定性测试（不依赖网络）。

use std::io::Write;
use std::process::{Command, Stdio};

use agenthub_core::model::McpServerDef;
use agenthub_core::runner::test_def;

const FAKE_SERVER: &str = r#"
import sys, json
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception:
        continue
    if req.get("method") == "initialize":
        resp = {
            "jsonrpc": "2.0",
            "id": req.get("id"),
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "serverInfo": {"name": "fake-mcp", "version": "1.2.3"},
            },
        }
        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()
    elif req.get("method") == "notifications/initialized":
        break
"#;

fn fake_server_def() -> McpServerDef {
    // python 写入临时文件后以文件为参数启动
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake_mcp.py");
    std::fs::write(&script, FAKE_SERVER).unwrap();
    // tempdir 在返回后会被清理，这里把脚本路径存到全局临时目录
    let persist = std::env::temp_dir().join("agenthub-test-fake_mcp.py");
    std::fs::copy(&script, &persist).unwrap();
    McpServerDef {
        command: Some("python".into()),
        args: vec![persist.display().to_string()],
        ..Default::default()
    }
}

#[test]
fn stdio_handshake_succeeds_with_fake_server() {
    let def = fake_server_def();
    let result = test_def(&def);
    assert_eq!(result.status, "ok", "应握手成功: {:?}", result.error);
    assert_eq!(result.server_name.as_deref(), Some("fake-mcp"));
    assert_eq!(result.server_version.as_deref(), Some("1.2.3"));
    assert!(result.latency_ms < 15_000);
}

#[test]
fn nonexistent_command_fails_fast_with_message() {
    let def = McpServerDef {
        command: Some("definitely-not-a-real-binary-xyz-9527".into()),
        args: vec![],
        ..Default::default()
    };
    let result = test_def(&def);
    assert_eq!(result.status, "failed");
    let msg = result.error.unwrap_or_default();
    assert!(
        msg.contains("无法启动") || msg.contains("提前退出"),
        "应给出可读错误: {msg}"
    );
}

#[test]
fn empty_def_reports_error() {
    let result = test_def(&McpServerDef::default());
    assert_eq!(result.status, "failed");
    assert!(result.error.unwrap_or_default().contains("command"));
}

#[test]
fn silent_process_times_out_or_exits() {
    // 一个不响应的进程：读 stdin 不回 → 超时或退出
    let dir = std::env::temp_dir().join("agenthub-test-silent.py");
    std::fs::write(&dir, "import sys\nfor line in sys.stdin:\n    pass\n").unwrap();
    let def = McpServerDef {
        command: Some("python".into()),
        args: vec![dir.display().to_string()],
        ..Default::default()
    };
    let result = test_def(&def);
    assert_eq!(result.status, "failed");
    let msg = result.error.unwrap_or_default();
    assert!(
        msg.contains("超时") || msg.contains("提前退出"),
        "无响应进程应超时或退出: {msg}"
    );
}

// 防止未使用告警（保留直接调用示例）
#[allow(dead_code)]
fn _unused(_: &mut Stdio) {}
#[allow(dead_code)]
fn _unused2<W: Write>(_: W) {}
