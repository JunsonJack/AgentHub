/**
 * 外部链接统一出口。
 *
 * 为什么要拦住所有链接：Tauri 只有一个窗口，没有地址栏也没有后退键。
 * 任何 http(s) 链接只要在 webview 里跳出去，整个应用就被换成了别人的网页，
 * 用户除了杀进程没有第二条回来的路。所以外部地址一律交给系统默认浏览器，
 * webview 永远停在 AgentHub 自己身上。
 *
 * installExternalLinkGuard() 在捕获阶段统一接管 <a> 点击，因此卡片链接、
 * SKILL.md 渲染出的 markdown 链接、以后新增的视图都不用各写一遍。
 */
import { ElMessage } from "element-plus";

/** 白名单协议；javascript: / data: / file: 等一律不放行（markdown 里的链接属外部输入） */
const SAFE_SCHEME = /^(?:https?:|mailto:|tel:)/i;

const inTauri = () => "__TAURI_INTERNALS__" in window;

/** 把地址交给系统默认浏览器；成功返回 true */
export async function openExternal(url: string): Promise<boolean> {
  const raw = (url || "").trim();
  if (!SAFE_SCHEME.test(raw)) {
    ElMessage.warning(`不支持打开的链接类型：${raw.slice(0, 40) || "(空)"}`);
    return false;
  }
  try {
    if (inTauri()) {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(raw);
    } else {
      // 纯浏览器里跑 vite 时（未嵌进 Tauri）退化为新标签页
      window.open(raw, "_blank", "noopener,noreferrer");
    }
    return true;
  } catch (e) {
    ElMessage.error(`打开链接失败：${e instanceof Error ? e.message : String(e)}`);
    return false;
  }
}

/**
 * 应用内部地址：同源的普通链接（hash 路由、页内锚点）。
 * 这类放行给 vue-router / 浏览器默认行为。
 */
function isAppInternal(a: HTMLAnchorElement): boolean {
  const raw = a.getAttribute("href") ?? "";
  if (!/^[a-z][a-z0-9+.-]*:/i.test(raw)) {
    // 相对地址、#hash：解析出的 origin 与应用一致即为内部
    return a.origin === window.location.origin;
  }
  return false;
}

let installed = false;

/** 安装全局外链守卫（幂等，main.ts 启动时调用一次） */
export function installExternalLinkGuard(): void {
  if (installed) return;
  installed = true;

  document.addEventListener(
    "click",
    (e: MouseEvent) => {
      // 只处理左键、且未被其它处理器消费掉的点击
      if (e.defaultPrevented || e.button !== 0) return;
      const a = (e.target as Element | null)?.closest?.("a[href]") as HTMLAnchorElement | null;
      if (!a || isAppInternal(a)) return;
      e.preventDefault(); // 关键：不让 webview 自己跳走
      void openExternal(a.href);
    },
    true, // 捕获阶段，早于任何组件自身的 click 处理
  );

  // window.open 也一并接管：第三方组件常拿它开外链
  const nativeOpen = window.open.bind(window);
  window.open = (url?: string | URL, target?: string, features?: string): Window | null => {
    if (url === undefined || url === "" || url === "about:blank") return nativeOpen(url, target, features);
    const raw = typeof url === "string" ? url : String(url);
    if (isAppInternalUrl(raw)) return nativeOpen(raw, target, features);
    void openExternal(raw);
    return null;
  };
}

function isAppInternalUrl(raw: string): boolean {
  if (!/^[a-z][a-z0-9+.-]*:/i.test(raw) && !raw.startsWith("//")) {
    return true; // 相对地址视为内部
  }
  try {
    return new URL(raw, window.location.href).origin === window.location.origin;
  } catch {
    return false;
  }
}
