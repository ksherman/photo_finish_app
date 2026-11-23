<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import QRCode from "qrcode";
import { formatCents, useKioskStore } from "../store";
import type { KioskOrder } from "../types";

const props = defineProps<{
  orderNumber: string;
}>();

const store = useKioskStore();
const order = ref<KioskOrder | null>(null);
const qrDataUrl = ref<string>("");
const loading = ref(true);

async function loadOrder() {
  loading.value = true;
  qrDataUrl.value = "";
  try {
    if (store.lastOrder?.orderNumber === props.orderNumber) {
      order.value = store.lastOrder;
    } else {
      const res = await fetch(`/api/orders/${encodeURIComponent(props.orderNumber)}`);
      if (res.ok) {
        order.value = await res.json();
      }
    }
    if (order.value?.squarePosUrl) {
      qrDataUrl.value = await QRCode.toDataURL(order.value.squarePosUrl, {
        margin: 2,
        width: 440,
        errorCorrectionLevel: "M",
      });
    }
  } finally {
    loading.value = false;
  }
}

function startNewOrder() {
  store.clearCart();
  store.lastOrder = null;
  store.navigate({ name: "home" });
}

onMounted(loadOrder);
watch(() => props.orderNumber, loadOrder);
</script>

<template>
  <div class="min-h-screen bg-slate-100 flex items-center justify-center p-4">
    <div v-if="loading" class="text-slate-500">Loading order...</div>

    <div
      v-else-if="order"
      class="max-w-xl w-full bg-white rounded-3xl shadow-xl border border-slate-200 p-7 sm:p-8 text-center space-y-6"
    >
      <div class="w-16 h-16 rounded-full bg-emerald-100 text-emerald-600 text-3xl font-bold flex items-center justify-center mx-auto">
        ✓
      </div>

      <div>
        <p class="text-sm font-semibold uppercase tracking-wider text-slate-400">Order Placed</p>
        <p class="text-5xl font-extrabold text-indigo-600 font-mono tracking-wide mt-1">
          {{ order.orderNumber }}
        </p>
        <p class="text-slate-600 mt-1">{{ order.customerName }}</p>
      </div>

      <!-- Offline Square POS QR Code -->
      <div class="bg-slate-50 rounded-2xl border border-slate-200 p-6 flex flex-col items-center gap-3">
        <div
          v-if="qrDataUrl"
          class="bg-white p-3 rounded-2xl border border-slate-200 shadow-xs inline-flex items-center justify-center"
        >
          <img
            :src="qrDataUrl"
            alt="Square POS Checkout QR Code"
            class="w-52 h-52 block"
          />
        </div>
        <p class="text-sm font-semibold text-slate-800">
          Show this QR code or order # at the checkout desk
        </p>
        <p class="text-2xl font-extrabold text-slate-900 font-mono">
          Total Due: {{ formatCents(order.totalCents) }}
        </p>
      </div>

      <!-- Compact Item & Tax Summary -->
      <div class="text-left border-t border-slate-200 pt-4 space-y-1.5 text-sm">
        <div v-for="item in order.items" :key="item.id" class="flex justify-between text-slate-600">
          <span class="truncate pr-3">
            {{ item.quantity > 1 ? `${item.quantity}× ` : "" }}{{ item.productName }}
            <span v-if="item.subjectDisplayName" class="text-slate-400">— {{ item.subjectDisplayName }}</span>
          </span>
          <span class="font-mono font-semibold text-slate-800">{{ formatCents(item.lineTotalCents) }}</span>
        </div>
        <div class="flex justify-between text-slate-500 pt-2 border-t border-slate-100">
          <span>Tax ({{ (order.taxRateBasisPoints / 100).toFixed(2) }}%)</span>
          <span class="font-mono">{{ formatCents(order.taxCents) }}</span>
        </div>
      </div>

      <button
        @click="startNewOrder"
        class="w-full py-4 rounded-2xl bg-slate-900 text-white text-lg font-bold hover:bg-slate-800 active:scale-[0.99] transition"
      >
        Done — Start New Order
      </button>
    </div>

    <div v-else class="bg-white rounded-2xl p-8 text-center space-y-4">
      <p class="text-slate-600">Order {{ orderNumber }} not found.</p>
      <button @click="startNewOrder" class="px-5 py-2.5 rounded-xl bg-indigo-600 text-white font-semibold">
        Back to Search
      </button>
    </div>
  </div>
</template>
