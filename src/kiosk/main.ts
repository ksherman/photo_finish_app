import { createApp } from "vue";
import { createPinia } from "pinia";
import KioskApp from "./KioskApp.vue";
import "@/styles.css";

const app = createApp(KioskApp);
app.use(createPinia());
app.mount("#kiosk-app");
