import { beforeEach, describe, expect, it, vi } from "vitest";
import { initTheme, useTheme } from "./theme";

// jsdom 没有 matchMedia，stub 一个可控实现。
// matches 用 getter：每次 applyTheme 读取时都拿到 darkPreferred 的当前值。
let darkPreferred = false;
Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    get matches() {
      return darkPreferred;
    },
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
  })),
});

beforeEach(() => {
  localStorage.clear();
  darkPreferred = false;
  document.documentElement.className = "";
  delete document.documentElement.dataset.theme;
  // 重新注册 mediaQuery 并应用当前主题，避免模块级单例状态在用例间串扰
  initTheme();
  useTheme().setTheme("system");
});

describe("useTheme", () => {
  it("默认跟随系统", () => {
    const { themeMode } = useTheme();
    expect(themeMode.value).toBe("system");
  });

  it("setTheme 持久化到 localStorage", () => {
    const { themeMode, setTheme } = useTheme();
    setTheme("dark");
    expect(themeMode.value).toBe("dark");
    expect(localStorage.getItem("agenthub-theme")).toBe("dark");
  });

  it("dark 模式给根元素加 dark class", () => {
    const { setTheme } = useTheme();
    setTheme("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("light 模式移除 dark class（即便系统偏好暗色）", () => {
    darkPreferred = true;
    const { setTheme } = useTheme();
    setTheme("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("system 模式跟随系统偏好", () => {
    const { setTheme } = useTheme();

    darkPreferred = true;
    setTheme("system");
    expect(document.documentElement.classList.contains("dark")).toBe(true);

    darkPreferred = false;
    setTheme("system");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });
});
