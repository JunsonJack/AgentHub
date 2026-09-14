//! frontmatter 解析器测试：折叠块、字面块、引号、注释、CRLF。

use agenthub_core::connector::parse_frontmatter_str;

#[test]
fn folded_scalar_multiline() {
    let md = "---\nname: auto-merge\ndescription: >\n  自动合并分支。\n  支持多个环境。\n  第二段落。\nversion: 1\n---\n\n# 正文\n";
    let (name, desc) = parse_frontmatter_str(md);
    assert_eq!(name.as_deref(), Some("auto-merge"));
    assert_eq!(
        desc.as_deref(),
        Some("自动合并分支。 支持多个环境。 第二段落。")
    );
}

#[test]
fn folded_scalar_with_trim_marker() {
    let md = "---\nname: x\ndescription: >-\n  first\n  second\n---\n";
    let (_, desc) = parse_frontmatter_str(md);
    assert_eq!(desc.as_deref(), Some("first second"));
}

#[test]
fn literal_scalar() {
    let md = "---\nname: x\ndescription: |\n  line one\n  line two\n---\n";
    let (_, desc) = parse_frontmatter_str(md);
    assert_eq!(desc.as_deref(), Some("line one line two"));
}

/// `description:` 后无值、接缩进续行（YAML plain 多行标量）。
/// 本机 ~/.agents/skills/web-access 就是这个写法，以前整段描述被丢成 None。
#[test]
fn plain_multiline_scalar_without_block_indicator() {
    let md = "---\nname: web-access\nlicense: MIT\ndescription:\n  所有联网操作必须通过此 skill 处理。\n  触发场景：用户要求搜索信息。\nmetadata:\n  author: someone\n---\n";
    let (name, desc) = parse_frontmatter_str(md);
    assert_eq!(name.as_deref(), Some("web-access"));
    assert_eq!(
        desc.as_deref(),
        Some("所有联网操作必须通过此 skill 处理。 触发场景：用户要求搜索信息。"),
        "缩进续行需折叠为一行，不得因含冒号而被误判成新键"
    );
}

/// 空值后紧跟顶格键：不得把后面的键吞进描述
#[test]
fn empty_value_then_sibling_key_is_not_folded() {
    let md = "---\ndescription:\nlicense: MIT\n---\n";
    let (_, desc) = parse_frontmatter_str(md);
    assert_eq!(desc, None, "无内容时仍为空，不应拼出垃圾值");
}

/// 顶格 name 不得被前一个空值块吸收
#[test]
fn plain_multiline_stops_at_first_unindented_line() {
    let md = "---\ndescription:\n  第一行\nname: real-name\n---\n";
    let (name, desc) = parse_frontmatter_str(md);
    assert_eq!(name.as_deref(), Some("real-name"));
    assert_eq!(desc.as_deref(), Some("第一行"));
}

#[test]
fn quoted_value_with_colon() {
    let md = "---\nname: \"use: when\"\ndescription: 'it''s fine: really'\n---\n";
    let (name, desc) = parse_frontmatter_str(md);
    assert_eq!(name.as_deref(), Some("use: when"));
    assert_eq!(desc.as_deref(), Some("it's fine: really"));
}

#[test]
fn inline_comment_stripped() {
    let md = "---\nname: x\ndescription: 快速提交 # v2 更新\n---\n";
    let (_, desc) = parse_frontmatter_str(md);
    assert_eq!(desc.as_deref(), Some("快速提交"));
}

#[test]
fn crlf_normalized() {
    let md = "---\r\nname: win\r\ndescription: hello world\r\n---\r\n";
    let (name, desc) = parse_frontmatter_str(md);
    assert_eq!(name.as_deref(), Some("win"));
    assert_eq!(desc.as_deref(), Some("hello world"));
}

#[test]
fn no_frontmatter_returns_none() {
    assert_eq!(parse_frontmatter_str("just text"), (None, None));
    assert_eq!(parse_frontmatter_str(""), (None, None));
}

#[test]
fn list_style_description_lines_join() {
    // 部分 skill 用 "- " 续行列表描述
    let md = "---\nname: x\ndescription: >\n  - 触发条件 A\n  - 触发条件 B\n---\n";
    let (_, desc) = parse_frontmatter_str(md);
    assert_eq!(desc.as_deref(), Some("触发条件 A 触发条件 B"));
}
