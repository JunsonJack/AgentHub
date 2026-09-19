import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown（DOMPurify 消毒边界）", () => {
  it("正常 markdown 转换为 html", () => {
    const html = renderMarkdown("# 标题\n\n- 列表项\n\n`code`");
    expect(html).toContain("<h1>");
    expect(html).toContain("<li>列表项</li>");
    expect(html).toContain("<code>code</code>");
  });

  it("空输入返回空串", () => {
    expect(renderMarkdown("")).toBe("");
    expect(renderMarkdown("   \n  ")).toBe("");
  });

  it("剥离 script 标签（XSS 防线）", () => {
    const html = renderMarkdown('<script>alert(1)</script>正文');
    expect(html).not.toContain("<script");
    expect(html).not.toContain("alert(1)");
    expect(html).toContain("正文");
  });

  it("剥离事件处理器（onclick 等）", () => {
    const html = renderMarkdown('<img src=x onerror="alert(1)">点击');
    expect(html).not.toContain("onerror");
    expect(html).toContain("点击");
  });

  it("剥离 javascript: 链接", () => {
    const html = renderMarkdown("[点我](javascript:alert(1))");
    expect(html).not.toContain("javascript:");
  });

  it("剥离 iframe / object / embed", () => {
    const html = renderMarkdown('<iframe src="https://evil.com"></iframe><object></object><embed src="x">');
    expect(html).not.toContain("<iframe");
    expect(html).not.toContain("<object");
    expect(html).not.toContain("<embed");
  });

  it("保留安全的普通链接与加粗", () => {
    const html = renderMarkdown("[GitHub](https://github.com) **加粗**");
    expect(html).toContain('href="https://github.com"');
    expect(html).toContain("<strong>加粗</strong>");
  });

  it("style 属性也被清理（防 CSS 注入）", () => {
    const html = renderMarkdown('<div style="position:fixed;top:0">x</div>');
    expect(html).not.toContain("style=");
  });
});
