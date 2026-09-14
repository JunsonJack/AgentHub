import { ref } from "vue";

export type ThemeMode = "system" | "light" | "dark";

const STORAGE_KEY = "agenthub-theme";
const themeMode = ref<ThemeMode>((localStorage.getItem(STORAGE_KEY) as ThemeMode) || "system");
let mediaQuery: MediaQueryList | null = null;

function applyTheme() {
  const dark = themeMode.value === "dark" || (themeMode.value === "system" && mediaQuery?.matches === true);
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.dataset.theme = themeMode.value;
}

export function useTheme() {
  function setTheme(mode: ThemeMode) {
    themeMode.value = mode;
    localStorage.setItem(STORAGE_KEY, mode);
    applyTheme();
  }

  return { themeMode, setTheme };
}

export function initTheme() {
  mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
  mediaQuery.addEventListener("change", applyTheme);
  applyTheme();
}
