import { marked } from "marked";
import DOMPurify from "dompurify";

marked.setOptions({ gfm: true, breaks: false });

/** SKILL.md 渲染：市场内容属外部输入，必须经 DOMPurify 消毒后再注入 */
export function renderMarkdown(md: string): string {
  if (!md || !md.trim()) return "";
  const html = marked.parse(md, { async: false });
  return DOMPurify.sanitize(typeof html === "string" ? html : "");
}
