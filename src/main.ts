import { createApp } from "vue";
import { createPinia } from "pinia";
import ElementPlus from "element-plus";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import "element-plus/dist/index.css";
import "./styles/apple-element.css";
import App from "./App.vue";
import router from "./router";
import { installExternalLinkGuard } from "./utils/external";
import { initTheme } from "./composables/theme";

// 启动即接管外链：应用窗口自身永不跳转到外部页面
installExternalLinkGuard();
initTheme();

createApp(App).use(createPinia()).use(router).use(ElementPlus, { locale: zhCn }).mount("#app");
