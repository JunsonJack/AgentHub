//! Skill 市场：skills.sh + SkillsMP 双源搜索与安装。
//!
//! 端点（2026-09 实测）：
//! - skills.sh 匿名搜索：GET /api/search?q=&limit=（CLI 同款内部端点，无需 Vercel OIDC）
//! - skills.sh 下载快照：GET /api/download/{owner}/{repo}/{slug} → { files: [{path, contents}], hash }
//! - SkillsMP 搜索：GET /api/v1/skills/search?q=&limit=（匿名 50 次/天；Bearer sk_live_* 500 次/天）
//!
//! 安装统一路径：取到 skill 目录（快照落盘 / git clone）→ 复用 library::adopt_into 入库。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::Deserialize;

use crate::error::{CoreError, Result};
use crate::library;
use crate::model::{AdoptReport, MarketPreview, MarketSkill};

const SKILLS_SH_BASE: &str = "https://skills.sh";
const SKILLSMP_BASE: &str = "https://skillsmp.com";
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/* ---------------- HTTP ---------------- */

fn agent_with_proxy(proxy: Option<ureq::Proxy>) -> ureq::Agent {    let mut b = ureq::AgentBuilder::new().timeout(HTTP_TIMEOUT);
    if let Some(p) = proxy {
        b = b.proxy(p);
    }
    b.build()
}

/// 代理策略：先读环境变量（Clash 等系统代理），失败则直连重试——
/// 国内网络下两类路径各有概率成功，双保险避免单点抖动导致功能不可用。
fn env_proxy() -> Option<ureq::Proxy> {
    ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy", "HTTP_PROXY", "http_proxy"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .filter(|v| !v.trim().is_empty())
        .and_then(|url| ureq::Proxy::new(url.trim()).ok())
}

fn http_get_json<T: for<'de> Deserialize<'de>>(url: &str, api_key: Option<&str>) -> Result<T> {
    let strategies = [env_proxy(), None];
    let mut last_err: Option<CoreError> = None;
    for proxy in strategies {
        match http_get_json_once(agent_with_proxy(proxy), url, api_key) {
            Ok(v) => return Ok(v),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| CoreError::Other("网络请求失败".into())))
}

fn http_get_json_once<T: for<'de> Deserialize<'de>>(
    agent: ureq::Agent,
    url: &str,
    api_key: Option<&str>,
) -> Result<T> {
    let req = agent.get(url);
    let req = match api_key {
        Some(k) if !k.trim().is_empty() => req.set("Authorization", &format!("Bearer {}", k.trim())),
        _ => req,
    };
    match req.call() {
        Ok(resp) => resp
            .into_json::<T>()
            .map_err(|e| CoreError::Other(format!("响应解析失败: {e}"))),
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            let hint = if body.len() > 200 { &body[..200] } else { &body };
            Err(CoreError::Other(format!("HTTP {code}: {hint}")))
        }
        Err(e) => Err(CoreError::Other(format!(
            "网络请求失败（请检查网络或代理设置）: {e}"
        ))),
    }
}

/// 连通性测试用：POST JSON（双策略降级与市场搜索一致）
pub(crate) fn post_json(url: &str, body: &str) -> Result<serde_json::Value> {
    let strategies = [env_proxy(), None];
    let mut last_err: Option<CoreError> = None;
    for proxy in strategies {
        let agent = agent_with_proxy(proxy);
        let res = agent
            .post(url)
            .set("Content-Type", "application/json")
            .set("Accept", "application/json, text/event-stream")
            .timeout(HTTP_TIMEOUT)
            .send_string(body);
        match res {
            Ok(resp) => {
                return resp
                    .into_json::<serde_json::Value>()
                    .map_err(|e| CoreError::Other(format!("响应解析失败: {e}")));
            }
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                // HTTP 状态码错误也可能是 JSON-RPC error 响应，尝试解析
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if v.get("jsonrpc").is_some() {
                        return Ok(v);
                    }
                }
                last_err = Some(CoreError::Other(format!("HTTP {code}")));
            }
            Err(e) => last_err = Some(CoreError::Other(format!("网络请求失败: {e}"))),
        }
    }
    Err(last_err.unwrap_or_else(|| CoreError::Other("网络请求失败".into())))
}

/// 连通性测试用：GET 状态码
pub(crate) fn get_status(url: &str) -> Result<u16> {
    let strategies = [env_proxy(), None];
    let mut last_err: Option<CoreError> = None;
    for proxy in strategies {
        match agent_with_proxy(proxy).get(url).call() {
            Ok(resp) => return Ok(resp.status()),
            Err(ureq::Error::Status(code, _)) => return Ok(code),
            Err(e) => last_err = Some(CoreError::Other(format!("网络请求失败: {e}"))),
        }
    }
    Err(last_err.unwrap_or_else(|| CoreError::Other("网络请求失败".into())))
}

/// 工具箱用：抓取网页 <title> 与 meta description（截断防爆）
pub fn fetch_url_metadata(url: &str) -> Result<(String, String)> {
    let strategies = [env_proxy(), None];
    let mut last_err: Option<CoreError> = None;
    for proxy in strategies {
        let res = agent_with_proxy(proxy)
            .get(url)
            .set("User-Agent", "Mozilla/5.0 AgentHub/0.1")
            .call();
        match res {
            Ok(resp) => {
                let html = resp.into_string().unwrap_or_default();
                let title = extract_tag(&html, "title").unwrap_or_default();
                let description = extract_meta_description(&html).unwrap_or_default();
                return Ok((
                    html_unescape(title.trim()),
                    html_unescape(description.trim()),
                ));
            }
            Err(e) => last_err = Some(CoreError::Other(format!("{e}"))),
        }
    }
    Err(last_err.unwrap_or_else(|| CoreError::Other("网络请求失败".into())))
}

fn extract_tag(html: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = html.to_lowercase().find(&open)?;
    let after_open = &html[start..];
    let content_start = after_open.find('>')? + 1;
    let close = after_open[content_start..].to_lowercase().find(&format!("</{tag}>"))?;
    Some(after_open[content_start..content_start + close].to_string())
}

fn extract_meta_description(html: &str) -> Option<String> {
    let lower = html.to_lowercase();
    let mut search = 0;
    while let Some(pos) = lower[search..].find("<meta") {
        let abs = search + pos;
        let end = html[abs..].find('>')? + abs;
        let tag = &html[abs..end];
        let name_ok = tag.to_lowercase().contains("description");
        let content = tag
            .to_lowercase()
            .find("content=")
            .and_then(|ci| {
                let rest = &tag[ci + 8..];
                let quote = rest.chars().next()?;
                if quote == '"' || quote == '\'' {
                    let close = rest[1..].find(quote)? + 1;
                    Some(rest[1..close].to_string())
                } else {
                    let end_pos = rest.find(|c: char| c == '>' || c == ' ')?;
                    Some(rest[..end_pos].to_string())
                }
            });
        if name_ok {
            if let Some(c) = content {
                return Some(c.chars().take(300).collect());
            }
        }
        search = end;
    }
    None
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/* ---------------- skills.sh ---------------- */

#[derive(Deserialize)]
struct SsSearch {
    #[serde(default)]
    skills: Vec<SsSkill>,
}

#[derive(Deserialize)]
struct SsSkill {
    id: String,
    name: String,
    #[serde(default)]
    installs: Option<u64>,
    #[serde(default)]
    source: Option<String>,
}

#[derive(Deserialize)]
struct SsDownload {
    #[serde(default)]
    files: Vec<SsFile>,
}

#[derive(Deserialize)]
struct SsFile {
    path: String,
    contents: String,
}

/// skills.sh 搜索（匿名）。id 形如 "owner/repo/slug"。
pub fn search_skills_sh(q: &str, limit: u32) -> Result<Vec<MarketSkill>> {
    let url = format!(
        "{}/api/search?q={}&limit={}",
        SKILLS_SH_BASE,
        urlencoding::encode(q),
        limit.clamp(1, 50)
    );
    let parsed: SsSearch = http_get_json(&url, None)?;
    Ok(parsed
        .skills
        .into_iter()
        .map(|s| MarketSkill {
            id: s.id.clone(),
            name: s.name,
            market: "skills.sh".into(),
            source: s.source,
            author: None,
            description: None,
            installs: s.installs,
            stars: None,
            github_url: None,
        })
        .collect())
}

/// 预览：拉取下载快照但只报告文件清单与描述（不落盘）
pub fn preview_skills_sh(id: &str) -> Result<MarketPreview> {
    let dl: SsDownload = http_get_json(&download_url(id)?, None)?;
    let skill_md = dl
        .files
        .iter()
        .find(|f| f.path == "SKILL.md" || f.path.ends_with("/SKILL.md"))
        .map(|f| f.contents.clone());
    let description = skill_md.and_then(|md| extract_desc_from_md(&md));
    Ok(MarketPreview {
        skill_name: id.rsplit('/').next().unwrap_or(id).to_string(),
        description,
        files: dl.files.into_iter().map(|f| f.path).collect(),
    })
}

/// 安装 skills.sh 条目：下载快照 → 暂存临时目录 → 复用 adopt 入库（含冲突拒绝）
/// 来源记为 `skills.sh:<id>`，供更新器识别上游
pub fn install_skills_sh(id: &str, data_root: &Path, dry_run: bool) -> Result<AdoptReport> {
    let fetched = fetch_skills_sh_to_temp(id)?;
    let result = library::adopt_into(
        data_root,
        &fetched.content_dir,
        &format!("skills.sh:{id}"),
        dry_run,
    );
    let _ = std::fs::remove_dir_all(&fetched.cleanup_root);
    result
}

fn download_url(id: &str) -> Result<String> {
    let parts: Vec<&str> = id.splitn(3, '/').collect();
    if parts.len() != 3 {
        return Err(CoreError::Other(format!("非法的 skills.sh 条目 id: {id}")));
    }
    Ok(format!(
        "{}/api/download/{}/{}/{}",
        SKILLS_SH_BASE,
        urlencoding::encode(parts[0]),
        urlencoding::encode(parts[1]),
        urlencoding::encode(parts[2])
    ))
}

/* ---------------- SkillsMP ---------------- */

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MpResponse {
    success: bool,
    #[serde(default)]
    data: Option<MpData>,
    #[serde(default)]
    error: Option<MpError>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MpData {
    #[serde(default)]
    skills: Vec<MpSkill>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MpError {
    code: Option<String>,
    message: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MpSkill {
    id: String,
    name: String,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    stars: Option<u64>,
    #[serde(default)]
    github_url: Option<String>,
}

/// SkillsMP 搜索。带 API 密钥可提升配额（500 次/天）并获得更好的功能向排序。
pub fn search_skillsmp(q: &str, limit: u32, api_key: Option<&str>) -> Result<Vec<MarketSkill>> {
    let url = format!(
        "{}/api/v1/skills/search?q={}&limit={}",
        SKILLSMP_BASE,
        urlencoding::encode(q),
        limit.clamp(1, 50)
    );
    let parsed: MpResponse = http_get_json(&url, api_key)?;
    if !parsed.success {
        let msg = parsed
            .error
            .map(|e| format!("{}: {}", e.code.unwrap_or_default(), e.message.unwrap_or_default()))
            .unwrap_or_else(|| "未知错误".into());
        return Err(CoreError::Other(format!("SkillsMP: {msg}")));
    }
    Ok(parsed
        .data
        .map(|d| d.skills)
        .unwrap_or_default()
        .into_iter()
        .map(|s| MarketSkill {
            id: s.id,
            name: s.name,
            market: "skillsmp".into(),
            source: None,
            author: s.author,
            description: s.description,
            installs: None,
            stars: s.stars,
            github_url: s.github_url,
        })
        .collect())
}

/// SkillsMP 条目安装：从 githubUrl（通常带子目录）git 安装
pub fn install_skillsmp(github_url: &str, data_root: &Path, dry_run: bool) -> Result<AdoptReport> {
    install_git(github_url, data_root, dry_run)
}

/* ---------------- Git URL 安装 ---------------- */

#[derive(Debug, Clone)]
pub struct GitSpec {
    pub repo_url: String,
    pub branch: Option<String>,
    pub subpath: Option<String>,
}

/// 解析 Git URL。支持：
/// - https://github.com/owner/repo
/// - https://github.com/owner/repo/tree/{branch}/{subpath}
/// - 任意 URL#subpath 片段
pub fn parse_git_url(input: &str) -> Result<GitSpec> {
    let raw = input.trim();
    if raw.is_empty() {
        return Err(CoreError::Other("URL 为空".into()));
    }
    let (main, frag_subpath) = match raw.split_once('#') {
        Some((m, f)) => (m.trim().trim_end_matches('/'), Some(f.trim().to_string())),
        None => (raw.trim().trim_end_matches('/'), None),
    };

    let (repo_url, branch, tree_subpath) = if let Some(pos) = main.find("/tree/") {
        let repo = &main[..pos];
        let rest = &main[pos + "/tree/".len()..];
        let mut segs = rest.split('/');
        let branch = segs.next().filter(|s| !s.is_empty()).map(String::from);
        let sub = segs.collect::<Vec<_>>().join("/");
        let sub = if sub.is_empty() { None } else { Some(sub) };
        (repo.to_string(), branch, sub)
    } else {
        (main.to_string(), None, None)
    };

    if !repo_url.starts_with("http://") && !repo_url.starts_with("https://") && !repo_url.contains(':') {
        // 允许 git@host:owner/repo 形式
        if !repo_url.starts_with("git@") {
            return Err(CoreError::Other(format!("无法识别的 Git URL: {repo_url}")));
        }
    }

    Ok(GitSpec {
        repo_url,
        branch,
        subpath: tree_subpath.or(frag_subpath),
    })
}

/// 从 Git URL 安装：浅克隆（必要时 sparse-checkout 子目录）→ 定位 skill 目录 → 入库。
/// dry-run 也会克隆（否则无从得知文件清单），但不会写入中央库。
pub fn install_git(url: &str, data_root: &Path, dry_run: bool) -> Result<AdoptReport> {
    let fetched = fetch_git_to_temp(url)?;
    let result = library::adopt_into(
        data_root,
        &fetched.content_dir,
        &format!("git:{}", parse_git_url(url)?.repo_url),
        dry_run,
    );
    let _ = std::fs::remove_dir_all(&fetched.cleanup_root);
    result
}

pub struct FetchedUpstream {
    /// 删除整个临时范围
    pub cleanup_root: PathBuf,
    pub skill_name: String,
    /// 以 skill 名命名的内容目录
    pub content_dir: PathBuf,
}

/// 拉取 Git 仓库（含子目录解析）到临时目录。调用方负责清理 cleanup_root。
pub fn fetch_git_to_temp(url: &str) -> Result<FetchedUpstream> {
    let spec = parse_git_url(url)?;
    let tmp = unique_temp_root("agenthub-clone");
    // 名字已包含进程号与自增序列；仍能撞上只可能是上轮崩溃的残留，
    // 不静默吞错（以前 `let _ = remove_dir_all` 会把 PermissionDenied 藏起来，
    // 导致下游 git 报出与真因无关的 confusing 错误）
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).map_err(|e| {
            CoreError::Other(format!(
                "临时克隆目录 {} 已存在且无法清理（可能被其它进程占用）: {e}",
                tmp.display()
            ))
        })?;
    }
    // 克隆到 tmp/src 子目录：仓库根安装时才能把 src 重命名为规范 skill 名
    let src = tmp.join("src");
    let mut cmd = Command::new("git");
    // 强制关闭 autocrlf：两次拉取的字节必须一致，否则更新检查会误报差异
    cmd.arg("-c").arg("core.autocrlf=false");
    cmd.arg("clone")
        .arg("--quiet")
        .arg("--depth")
        .arg("1")
        .arg("--filter=blob:none");
    if let Some(b) = &spec.branch {
        cmd.arg("--branch").arg(b);
    }
    if spec.subpath.is_some() {
        cmd.arg("--sparse");
    }
    cmd.arg(&spec.repo_url).arg(&src);
    run_git(&mut cmd)?;

    if let Some(sp) = &spec.subpath {
        run_git(
            Command::new("git")
                .current_dir(&tmp)
                .args(["sparse-checkout", "set", sp]),
        )?;
    }
    let skill_dir = locate_skill_dir(&src, spec.subpath.as_deref())?;
    let (skill_name, content_dir) = if skill_dir == src {
        // 仓库根即 skill：以仓库名命名（GitHub 场景即 repo 名）
        let name = repo_name(&spec.repo_url);
        let renamed = tmp.join(&name);
        std::fs::rename(&src, &renamed)?;
        (name, renamed)
    } else {
        let name = skill_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "skill".into());
        (name, skill_dir)
    };
    // 剥离 .git：中央库不需要历史（更新走重新拉取），且 .git 内部文件会让 diff 误报
    let git_dir = content_dir.join(".git");
    if git_dir.exists() {
        let _ = std::fs::remove_dir_all(&git_dir);
    }
    Ok(FetchedUpstream {
        cleanup_root: tmp,
        skill_name,
        content_dir,
    })
}

/// 从仓库 URL 取规范 skill 名（去 .git 后缀，替换文件系统不安全字符）
fn repo_name(url: &str) -> String {
    let normalized = url.trim_end_matches('/').replace('\\', "/");
    let name = normalized.rsplit('/').next().unwrap_or("skill");
    let name = name.strip_suffix(".git").unwrap_or(name);
    name.chars()
        .map(|c| {
            if matches!(c, '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '-'
            } else {
                c
            }
        })
        .collect()
}

/// 拉取 skills.sh 快照到临时目录。调用方负责清理 cleanup_root。
pub fn fetch_skills_sh_to_temp(id: &str) -> Result<FetchedUpstream> {
    let dl: SsDownload = http_get_json(&download_url(id)?, None)?;
    if dl.files.is_empty() {
        return Err(CoreError::Other("市场返回的快照为空".into()));
    }
    let name = id.rsplit('/').next().unwrap_or(id).to_string();
    // 暂存：<tmp>/<随机目录>/<name> —— 内层以 skill 名命名，入库目录名才是规范名
    let stage_root = staging_dir()?;
    let stage = stage_root.join(&name);
    std::fs::create_dir_all(&stage)?;
    for f in &dl.files {
        let rel = Path::new(&f.path);
        if rel.is_absolute() || f.path.contains("..") {
            let _ = std::fs::remove_dir_all(&stage_root);
            return Err(CoreError::Other(format!("快照含非法路径: {}", f.path)));
        }
        let dest = stage.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(dest, &f.contents)?;
    }
    Ok(FetchedUpstream {
        cleanup_root: stage_root,
        skill_name: name,
        content_dir: stage,
    })
}

fn run_git(cmd: &mut Command) -> Result<()> {
    let out = cmd.output().map_err(|e| {
        CoreError::Other(format!(
            "无法执行 git（请确认已安装并在 PATH 中）: {e}"
        ))
    })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(CoreError::Other(format!(
            "git 失败: {}",
            stderr.lines().last().unwrap_or("未知错误").trim()
        )));
    }
    Ok(())
}

/// 克隆后定位 skill 目录：指定子路径 → 根有 SKILL.md → 唯一含 SKILL.md 的子目录
fn locate_skill_dir(clone_root: &Path, subpath: Option<&str>) -> Result<PathBuf> {
    if let Some(sp) = subpath {
        let dir = clone_root.join(sp);
        if !dir.is_dir() {
            return Err(CoreError::NotFound(format!("子路径不存在: {sp}")));
        }
        return Ok(dir);
    }
    if clone_root.join("SKILL.md").is_file() {
        return Ok(clone_root.to_path_buf());
    }
    let candidates: Vec<PathBuf> = std::fs::read_dir(clone_root)
        .map_err(CoreError::from)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join("SKILL.md").is_file())
        .collect();
    match candidates.len() {
        1 => Ok(candidates[0].clone()),
        0 => Err(CoreError::Other(
            "仓库里没有找到 SKILL.md；如果 skill 在子目录，请用 URL#子路径 或 /tree/ 链接指定".into(),
        )),
        _ => {
            let names = candidates
                .iter()
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .collect::<Vec<_>>()
                .join("、");
            Err(CoreError::Other(format!(
                "仓库含多个 skill（{names}），请用 URL#子路径 指定其一"
            )))
        }
    }
}

/* ---------------- 工具 ---------------- */

static STAGE_SEQ: AtomicU64 = AtomicU64::new(0);

/// 生成进程内绝不重名、跨进程几乎不重名的临时目录名。
///
/// 以前只用毫秒时间戳：并发的安装 / 更新检查落在同一毫秒会撞同一个目录，
/// Windows 上表现为 remove_dir_all 报 PermissionDenied（被 `let _ =` 吞掉）
/// 后 git clone 撞残留路径报 "already exists and is not an empty directory"；
/// staging 目录更隐蔽：create_dir_all 遇到已存在目录会静默复用，
/// 两个并发任务会在同一目录里互相污染快照。毫秒 + 进程号 + 自增序列消除这种碰撞。
fn unique_temp_root(prefix: &str) -> PathBuf {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "{prefix}-{ms}-{}-{}",
        std::process::id(),
        STAGE_SEQ.fetch_add(1, Ordering::Relaxed)
    ))
}

fn staging_dir() -> Result<PathBuf> {
    // create_dir 在目录已存在时会原子失败，刚好当唯一性哨兵：
    // 避免以前 create_dir_all 静默复用同一目录、并发任务互相污染快照
    for _ in 0..8 {
        let dir = unique_temp_root("agenthub-stage");
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(CoreError::Other(format!(
                    "创建临时目录 {} 失败: {e}",
                    dir.display()
                )))
            }
        }
    }
    Err(CoreError::Other("连续多次获取唯一临时目录失败".into()))
}

fn extract_desc_from_md(md: &str) -> Option<String> {
    crate::connector::parse_frontmatter_str(md).1
}
