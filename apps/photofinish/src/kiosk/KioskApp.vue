<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { useKioskStore } from "./store";
import KioskHomeView from "./views/KioskHomeView.vue";
import KioskSubjectView from "./views/KioskSubjectView.vue";
import KioskCartView from "./views/KioskCartView.vue";
import KioskCheckoutView from "./views/KioskCheckoutView.vue";
import KioskConfirmationView from "./views/KioskConfirmationView.vue";

const store = useKioskStore();

function onPopState() {
  store.syncRouteFromWindow();
}

onMounted(() => {
  store.bootstrap();
  window.addEventListener("popstate", onPopState);
});

onUnmounted(() => {
  window.removeEventListener("popstate", onPopState);
});
</script>

<template>
  <div v-if="store.loading && !store.activeEvent" class="min-h-screen bg-slate-950 text-white flex items-center justify-center">
    <p class="text-slate-400 text-lg">Connecting to PhotoFinish server...</p>
  </div>

  <div
    v-else-if="!store.activeEvent"
    class="min-h-screen bg-slate-950 text-white flex items-center justify-center p-6 text-center"
  >
    <div class="max-w-md space-y-3">
      <h1 class="text-3xl font-extrabold">PhotoFinish Kiosk</h1>
      <p class="text-slate-400">Waiting for operator to open an event on the station...</p>
      <button
        @click="store.bootstrap()"
        class="mt-2 px-5 py-2.5 rounded-xl bg-indigo-600 text-white font-semibold"
      >
        Retry
      </button>
    </div>
  </div>

  <KioskHomeView v-else-if="store.route.name === 'home'" />
  <KioskSubjectView v-else-if="store.route.name === 'subject'" :subject-id="store.route.subjectId" />
  <KioskCartView v-else-if="store.route.name === 'cart'" />
  <KioskCheckoutView v-else-if="store.route.name === 'checkout'" />
  <KioskConfirmationView v-else :order-number="store.route.orderNumber" />
</template>
