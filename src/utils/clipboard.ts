import { ElMessage } from "element-plus";

/**
 * 复制文本。webview 里 navigator.clipboard 需要安全上下文，
 * 拿不到时退化到 execCommand，保证"复制链接"这类小动作不会静默失败。
 */
export async function copyText(text: string, okTip = "已复制"): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      ElMessage.success(okTip);
      return true;
    }
  } catch {
    /* 落到下面的兜底路径 */
  }
  try {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "");
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    document.body.removeChild(ta);
    if (ok) {
      ElMessage.success(okTip);
    } else {
      ElMessage.error("复制失败，请手动选择文本");
    }
    return ok;
  } catch {
    ElMessage.error("复制失败，请手动选择文本");
    return false;
  }
}
