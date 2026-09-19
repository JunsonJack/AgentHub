import { marked } from "marked";
import DOMPurify from "dompurify";

marked.setOptions({ gfm: true, breaks: false });

/** SKILL.md 渲染：市场内容属外部输入，必须经 DOMPurify 消毒后再注入。
 *  除默认过滤外额外禁掉 style 属性与目标超能力，防止 CSS 注入与页面劫持。 */
export function renderMarkdown(md: string): string {
  if (!md || !md.trim()) return "";
  const html = marked.parse(md, { async: false });
  return DOMPurify.sanitize(typeof html === "string" ? html : "", {
    FORBID_ATTR: ["style"],
    FORBID_TAGS: ["style", "iframe", "object", "embed", "form"],
  });
}
