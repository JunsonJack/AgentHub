//! 市场冒烟：真实网络搜索 + 下载安装到临时目录（不影响本机任何配置）。
//! 运行：cargo run -p agenthub-core --example market_smoke

use agenthub_core::market;

fn main() {
    println!("=== 1. skills.sh 搜索 \"git\"（匿名） ===");
    let rs = market::search_skills_sh("git", 5).expect("skills.sh 搜索失败");
    for s in &rs {
        println!(
            "{:<30} {:>9} 安装  {}",
            s.name,
            s.installs.unwrap_or(0),
            s.id
        );
    }

    println!("\n=== 2. skills.sh 预览 + 真实下载安装到临时库 ===");
    let first = rs.first().expect("应有结果");
    let pv = market::preview_skills_sh(&first.id).expect("预览失败");
    println!("skill: {}  文件: {:?}", pv.skill_name, pv.files);
    println!(
        "描述: {}",
        pv.description.as_deref().unwrap_or("-").chars().take(80).collect::<String>()
    );

    let data = tempfile::tempdir().expect("tempdir");
    let report = market::install_skills_sh(&first.id, data.path(), false).expect("安装失败");
    println!(
        "已入库: {}（{} 个文件，冲突: {}）→ {}",
        report.skill_name,
        report.files.len(),
        report.conflict,
        report.target_dir
    );

    println!("\n=== 3. SkillsMP 搜索 \"automation\"（匿名） ===");
    let ms = market::search_skillsmp("automation", 3, None).expect("SkillsMP 搜索失败");
    for s in &ms {
        println!("{:<30} ★{}", s.name, s.stars.unwrap_or(0));
        println!("   {}", s.github_url.as_deref().unwrap_or("-"));
    }

    if let Some(s) = ms.iter().find(|s| s.github_url.is_some()) {
        println!("\n=== 4. Git 路径安装（sparse clone）: {} ===", s.name);
        let report = market::install_git(s.github_url.as_deref().unwrap(), data.path(), false);
        match report {
            Ok(r) => println!(
                "已入库: {}（{} 个文件）→ {}",
                r.skill_name,
                r.files.len(),
                r.target_dir
            ),
            Err(e) => println!("git 安装失败（网络/仓库因素可接受）: {e}"),
        }
    }

    println!("\n冒烟完成，临时目录：{}", data.path().display());
}
