<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import QRCode from "qrcode";
import { useEventStore } from "@/stores/event";
import StatusBanners from "@/components/StatusBanners.vue";
import OrderUsbFulfillmentPanel from "@/components/orders/OrderUsbFulfillmentPanel.vue";
import PricingCatalogPanel from "@/components/orders/PricingCatalogPanel.vue";
import { exportOrdersCsvDialog } from "@/utils/csv";
import {
  formatCents,
  formatShippingAddress,
  formatTime,
  paymentMethodLabel,
  paymentStatusClass,
} from "@/utils/format";
import type { OrderItemRecord, OrderListResponse, OrderRecord, UsbDriveInfo } from "@/types";

const eventStore = useEventStore();

type SubTab = "desk" | "pricing";
type QueueFilter = "all" | "pending_payment" | "needs_usb" | "paid" | "pending_fulfillment" | "refunded";

const QUEUE_FILTERS: ReadonlyArray<{ id: QueueFilter; label: string }> = [
  { id: "all", label: "All" },
  { id: "pending_payment", label: "Pending Pay" },
  { id: "needs_usb", label: "Needs USB" },
  { id: "paid", label: "Paid" },
  { id: "pending_fulfillment", label: "Open Items" },
  { id: "refunded", label: "Refunded" },
];

const PAYMENT_METHODS = [
  { id: "square_qr", label: "Square QR" },
  { id: "square_terminal", label: "Square Terminal" },
  { id: "cash", label: "Cash" },
  { id: "check", label: "Check" },
  { id: "other", label: "Other" },
] as const;

const subTab = ref<SubTab>("desk");
const queueFilter = ref<QueueFilter>("all");
const searchQuery = ref("");
const pricingRef = ref<InstanceType<typeof PricingCatalogPanel> | null>(null);

const orderData = ref<OrderListResponse>({
  stats: {
    totalOrders: 0,
    pendingPaymentCount: 0,
    paidCount: 0,
    refundedCount: 0,
    pendingUsbCount: 0,
    pendingFulfillmentCount: 0,
    grossPaidCents: 0,
    subtotalPaidCents: 0,
    shippingPaidCents: 0,
    taxPaidCents: 0,
    pendingRevenueCents: 0,
  },
  orders: [],
});

const selectedOrderId = ref<string | null>(null);
const bannerMessage = ref<string | null>(null);
const errorMessage = ref<string | null>(null);

const paymentMethod = ref("square_qr");
const paymentRefInput = ref("");
const notesInput = ref("");
const isSavingPayment = ref(false);
const isSavingNotes = ref(false);
const showQrModal = ref(false);
const qrDataUrl = ref("");

const usbDrives = ref<UsbDriveInfo[]>([]);
const isCopyingUsb = ref(false);

let ordersPollTimer: ReturnType<typeof setInterval> | null = null;
let usbPollTimer: ReturnType<typeof setInterval> | null = null;

const thumbBaseUrl = computed(() => `http://127.0.0.1:${eventStore.lanStatus.port || 4000}/photos/thumbnail`);
const readyUsbCount = computed(() => usbDrives.value.filter((d) => !d.hasDcim).length);

const hasUsbAddon = (o: OrderRecord) => o.items.some((i) => i.productType === "usb_addon");
const hasPendingUsb = (o: OrderRecord) =>
  o.paymentStatus !== "refunded" &&
  o.items.some((i) => i.productType === "usb_addon" && i.fulfillmentStatus !== "fulfilled");
const hasPrints = (o: OrderRecord) => o.items.some((i) => i.productType === "print");
const hasDigitalPhotos = (o: OrderRecord) =>
  o.items.some((i) => ["full_digital", "single_digital", "usb_addon"].includes(i.productType));
const isFullyFulfilled = (o: OrderRecord) =>
  o.items.length > 0 && o.items.every((i) => i.fulfillmentStatus === "fulfilled");

const filteredOrders = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  return orderData.value.orders.filter((ord) => {
    if (queueFilter.value === "pending_payment" && ord.paymentStatus !== "pending") return false;
    if (queueFilter.value === "paid" && ord.paymentStatus !== "paid") return false;
    if (queueFilter.value === "refunded" && ord.paymentStatus !== "refunded") return false;
    if (queueFilter.value === "needs_usb" && !hasPendingUsb(ord)) return false;
    if (queueFilter.value === "pending_fulfillment" && (ord.paymentStatus === "refunded" || isFullyFulfilled(ord))) {
      return false;
    }
    if (!q) return true;
    return (
      ord.orderNumber.toLowerCase().includes(q) ||
      ord.customerName.toLowerCase().includes(q) ||
      (ord.customerEmail || "").toLowerCase().includes(q) ||
      (ord.customerPhone || "").toLowerCase().includes(q) ||
      ord.items.some(
        (i) =>
          (i.subjectDisplayName || "").toLowerCase().includes(q) ||
          i.productName.toLowerCase().includes(q)
      )
    );
  });
});

const selectedOrder = computed<OrderRecord | null>(
  () => orderData.value.orders.find((o) => o.id === selectedOrderId.value) || null
);

async function loadOrders(silent = false) {
  if (!eventStore.activeEvent) return;
  try {
    const res = await invoke<OrderListResponse>("list_orders");
    const prevCount = orderData.value.orders.length;
    orderData.value = res;
    if (res.orders.length !== prevCount) eventStore.refreshActiveEvent();
    if (!selectedOrderId.value || !res.orders.some((o) => o.id === selectedOrderId.value)) {
      selectedOrderId.value = res.orders[0]?.id || null;
    }
  } catch (err) {
    if (!silent) errorMessage.value = String(err);
  }
}

async function refreshUsbDrives() {
  try {
    usbDrives.value = await invoke<UsbDriveInfo[]>("discover_usb_drives");
  } catch {
    // Non-fatal on unsupported OS
  }
}

async function onUsbCopied(msg: string) {
  errorMessage.value = null;
  bannerMessage.value = msg;
  await Promise.all([loadOrders(true), refreshUsbDrives()]);
}

async function submitPaymentStatus(status: "paid" | "pending" | "refunded") {
  const ord = selectedOrder.value;
  if (!ord || isSavingPayment.value) return;
  isSavingPayment.value = true;
  errorMessage.value = null;
  bannerMessage.value = null;
  try {
    await invoke<OrderRecord>("update_order_payment", {
      request: {
        orderId: ord.id,
        paymentStatus: status,
        paymentMethod: status === "pending" ? null : paymentMethod.value,
        paymentReference: status === "pending" ? null : paymentRefInput.value.trim() || null,
        notes: notesInput.value,
      },
    });
    bannerMessage.value =
      status === "paid"
        ? `Order ${ord.orderNumber} marked PAID (${formatCents(ord.totalCents)} via ${paymentMethodLabel(paymentMethod.value)}).`
        : status === "refunded"
          ? `Order ${ord.orderNumber} marked REFUNDED.`
          : `Order ${ord.orderNumber} reverted to Pending Payment.`;
    await loadOrders(true);
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isSavingPayment.value = false;
  }
}

async function saveOrderNotes() {
  const ord = selectedOrder.value;
  if (!ord || isSavingNotes.value) return;
  isSavingNotes.value = true;
  errorMessage.value = null;
  try {
    await invoke<OrderRecord>("update_order_notes", {
      orderId: ord.id,
      notes: notesInput.value.trim() || null,
    });
    bannerMessage.value = `Saved notes for ${ord.orderNumber}.`;
    await loadOrders(true);
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isSavingNotes.value = false;
  }
}

async function toggleItemFulfillment(item: OrderItemRecord) {
  const ord = selectedOrder.value;
  if (!ord) return;
  errorMessage.value = null;
  try {
    await invoke<OrderRecord>("update_order_item_fulfillment", {
      orderId: ord.id,
      itemId: item.id,
      fulfillmentStatus: item.fulfillmentStatus === "fulfilled" ? "pending" : "fulfilled",
    });
    await loadOrders(true);
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function openQrModal() {
  const ord = selectedOrder.value;
  if (!ord) return;
  qrDataUrl.value = await QRCode.toDataURL(ord.squarePosUrl, {
    margin: 2,
    width: 440,
    errorCorrectionLevel: "M",
  });
  showQrModal.value = true;
}

async function handleExportCsv(allLibraryEvents: boolean) {
  errorMessage.value = null;
  bannerMessage.value = null;
  try {
    const msg = await exportOrdersCsvDialog({
      allLibraryEvents,
      defaultFilename: allLibraryEvents
        ? "photofinish-all-events-orders.csv"
        : `${eventStore.activeEvent?.slug || "event"}-orders.csv`,
      title: allLibraryEvents
        ? "Export Consolidated Orders CSV (All Events)"
        : "Export Event Orders CSV",
    });
    if (msg) bannerMessage.value = msg;
  } catch (err) {
    errorMessage.value = String(err);
  }
}

watch(
  selectedOrder,
  (ord, prev) => {
    if (ord && (!prev || ord.id !== prev.id)) {
      paymentMethod.value = ord.paymentMethod || "square_qr";
      paymentRefInput.value = ord.paymentReference || "";
      notesInput.value = ord.notes || "";
    }
  },
  { immediate: true }
);

watch(
  () => eventStore.activeEvent?.id,
  () => {
    selectedOrderId.value = null;
    if (eventStore.activeEvent) loadOrders();
  }
);

onMounted(() => {
  if (eventStore.activeEvent) loadOrders();
  refreshUsbDrives();
  ordersPollTimer = setInterval(() => {
    if (eventStore.activeEvent && subTab.value === "desk") loadOrders(true);
  }, 5000);
  usbPollTimer = setInterval(() => {
    if (subTab.value === "desk" && !isCopyingUsb.value) refreshUsbDrives();
  }, 3500);
});

onUnmounted(() => {
  if (ordersPollTimer) clearInterval(ordersPollTimer);
  if (usbPollTimer) clearInterval(usbPollTimer);
});
</script>

<template>
  <div class="max-w-7xl mx-auto p-6 space-y-5">
    <div v-if="!eventStore.activeEvent" class="p-4 rounded-xl bg-amber-50 border border-amber-200 text-sm text-amber-800">
      Open or create an Event Bundle first to manage orders, Square checkout payments, and instant USB fulfillment.
    </div>

    <template v-else>
      <!-- Header -->
      <div class="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h1 class="text-2xl font-bold text-slate-900">Orders &amp; USB Fulfillment</h1>
          <p class="text-sm text-slate-500 mt-0.5">
            Live kiosk order queue, Square POS payment reconciliation, one-click USB-C/A fulfillment, and CSV reporting.
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-2">
          <div class="inline-flex rounded-lg bg-slate-200 p-1 text-sm font-medium">
            <button
              @click="subTab = 'desk'"
              class="px-3.5 py-1.5 rounded-md transition-colors flex items-center gap-1.5"
              :class="subTab === 'desk' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
            >
              Order Desk
              <span
                v-if="orderData.stats.pendingPaymentCount + orderData.stats.pendingUsbCount > 0"
                class="px-1.5 py-0.5 text-xs rounded-full bg-indigo-100 text-indigo-800"
              >
                {{ orderData.stats.totalOrders }}
              </span>
            </button>
            <button
              @click="subTab = 'pricing'; pricingRef?.loadCatalog()"
              class="px-3.5 py-1.5 rounded-md transition-colors"
              :class="subTab === 'pricing' ? 'bg-white text-slate-900 shadow-sm' : 'text-slate-600 hover:text-slate-900'"
            >
              Pricing &amp; Tax
            </button>
          </div>

          <button
            @click="handleExportCsv(false)"
            class="px-3 py-2 text-xs font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
          >
            Export Event CSV
          </button>
          <button
            @click="handleExportCsv(true)"
            class="px-3 py-2 text-xs font-medium rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
            title="Consolidate orders across all Event Bundles currently in your Library"
          >
            Export All Events CSV
          </button>
        </div>
      </div>

      <StatusBanners
        :banner-message="bannerMessage"
        :error-message="errorMessage"
        @clear-banner="bannerMessage = null"
        @clear-error="errorMessage = null"
      />

      <!-- ================= ORDER DESK TAB ================= -->
      <div v-show="subTab === 'desk'" class="space-y-4">
        <!-- KPI Summary Cards -->
        <div class="grid grid-cols-2 lg:grid-cols-4 gap-3">
          <button
            @click="queueFilter = queueFilter === 'pending_payment' ? 'all' : 'pending_payment'"
            class="text-left p-4 rounded-xl border transition-all"
            :class="queueFilter === 'pending_payment' ? 'border-amber-400 bg-amber-50 ring-2 ring-amber-200' : 'border-slate-200 bg-white hover:border-slate-300'"
          >
            <div class="text-xs font-medium text-slate-500 uppercase tracking-wide">Awaiting Payment</div>
            <div class="mt-1 flex items-baseline justify-between">
              <span class="text-2xl font-bold" :class="orderData.stats.pendingPaymentCount > 0 ? 'text-amber-700' : 'text-slate-900'">
                {{ orderData.stats.pendingPaymentCount }}
              </span>
              <span class="text-xs font-mono text-slate-500">{{ formatCents(orderData.stats.pendingRevenueCents) }}</span>
            </div>
          </button>

          <button
            @click="queueFilter = queueFilter === 'needs_usb' ? 'all' : 'needs_usb'"
            class="text-left p-4 rounded-xl border transition-all"
            :class="queueFilter === 'needs_usb' ? 'border-indigo-400 bg-indigo-50 ring-2 ring-indigo-200' : 'border-slate-200 bg-white hover:border-slate-300'"
          >
            <div class="text-xs font-medium text-slate-500 uppercase tracking-wide">Needs USB Copy</div>
            <div class="mt-1 flex items-baseline justify-between">
              <span class="text-2xl font-bold" :class="orderData.stats.pendingUsbCount > 0 ? 'text-indigo-700' : 'text-slate-900'">
                {{ orderData.stats.pendingUsbCount }}
              </span>
              <span class="text-xs text-slate-500">
                {{ readyUsbCount }} USB drive{{ readyUsbCount === 1 ? "" : "s" }} ready
              </span>
            </div>
          </button>

          <button
            @click="queueFilter = queueFilter === 'paid' ? 'all' : 'paid'"
            class="text-left p-4 rounded-xl border transition-all"
            :class="queueFilter === 'paid' ? 'border-emerald-400 bg-emerald-50 ring-2 ring-emerald-200' : 'border-slate-200 bg-white hover:border-slate-300'"
          >
            <div class="text-xs font-medium text-slate-500 uppercase tracking-wide">Paid Revenue</div>
            <div class="mt-1 flex items-baseline justify-between">
              <span class="text-2xl font-bold text-emerald-700 font-mono">
                {{ formatCents(orderData.stats.grossPaidCents) }}
              </span>
              <span class="text-xs text-slate-500">{{ orderData.stats.paidCount }} paid</span>
            </div>
          </button>

          <button
            @click="queueFilter = 'all'"
            class="text-left p-4 rounded-xl border transition-all"
            :class="queueFilter === 'all' ? 'border-slate-400 bg-slate-50 ring-2 ring-slate-200' : 'border-slate-200 bg-white hover:border-slate-300'"
          >
            <div class="text-xs font-medium text-slate-500 uppercase tracking-wide">Total Orders</div>
            <div class="mt-1 flex items-baseline justify-between">
              <span class="text-2xl font-bold text-slate-900">{{ orderData.stats.totalOrders }}</span>
              <span class="text-xs text-slate-500">{{ orderData.stats.pendingFulfillmentCount }} open</span>
            </div>
          </button>
        </div>

        <!-- Two-Pane Order Desk -->
        <div class="grid grid-cols-1 lg:grid-cols-12 gap-5">
          <!-- Left: Order Queue -->
          <div class="lg:col-span-5 bg-white rounded-xl border border-slate-200 flex flex-col max-h-[68vh] overflow-hidden">
            <div class="p-3.5 bg-slate-50 border-b border-slate-200 space-y-2.5">
              <div class="flex items-center gap-2">
                <input
                  v-model="searchQuery"
                  type="search"
                  placeholder="Search order #, customer, or athlete..."
                  class="flex-1 border border-slate-300 bg-white rounded-lg px-3 py-1.5 text-sm"
                />
                <button
                  @click="loadOrders()"
                  class="px-2.5 py-1.5 text-xs font-medium rounded-lg border border-slate-300 bg-white text-slate-600 hover:bg-slate-100"
                >
                  Refresh
                </button>
              </div>
              <div class="flex flex-wrap gap-1 text-xs">
                <button
                  v-for="f in QUEUE_FILTERS"
                  :key="f.id"
                  @click="queueFilter = f.id"
                  class="px-2.5 py-1 rounded-full font-medium transition-colors"
                  :class="queueFilter === f.id ? 'bg-slate-900 text-white' : 'bg-white border border-slate-200 text-slate-600 hover:bg-slate-100'"
                >
                  {{ f.label }}
                </button>
              </div>
            </div>

            <div class="divide-y divide-slate-100 overflow-y-auto flex-1">
              <button
                v-for="ord in filteredOrders"
                :key="ord.id"
                @click="selectedOrderId = ord.id"
                class="w-full p-3.5 text-left transition-colors space-y-1.5"
                :class="selectedOrderId === ord.id ? 'bg-blue-50/80 border-l-4 border-l-blue-600' : 'hover:bg-slate-50 border-l-4 border-l-transparent'"
              >
                <div class="flex items-center justify-between gap-2">
                  <div class="flex items-center gap-2 min-w-0">
                    <span class="font-mono text-sm font-bold text-indigo-700">{{ ord.orderNumber }}</span>
                    <span class="text-sm font-semibold text-slate-900 truncate">{{ ord.customerName }}</span>
                  </div>
                  <span class="font-mono text-sm font-bold text-slate-900 flex-shrink-0">
                    {{ formatCents(ord.totalCents) }}
                  </span>
                </div>

                <div class="flex items-center justify-between gap-2 text-xs">
                  <div class="flex items-center flex-wrap gap-1.5">
                    <span class="px-2 py-0.5 rounded-full font-semibold" :class="paymentStatusClass(ord.paymentStatus)">
                      {{ ord.paymentStatus === "pending" ? "Pending Pay" : ord.paymentStatus === "paid" ? "Paid" : "Refunded" }}
                    </span>
                    <span v-if="hasPendingUsb(ord)" class="px-2 py-0.5 rounded-full bg-indigo-100 text-indigo-800 font-semibold">
                      USB Needed
                    </span>
                    <span v-else-if="hasUsbAddon(ord)" class="px-2 py-0.5 rounded-full bg-slate-100 text-slate-600">
                      USB Done ✓
                    </span>
                    <span v-if="hasPrints(ord)" class="px-2 py-0.5 rounded-full bg-purple-100 text-purple-800 font-medium">
                      Prints
                    </span>
                  </div>
                  <span class="text-slate-400 flex-shrink-0">{{ formatTime(ord.createdAt) }}</span>
                </div>

                <div class="text-xs text-slate-500 truncate">
                  {{ ord.items.map((i) => `${i.productName}${i.subjectDisplayName ? ` (${i.subjectDisplayName})` : ""}`).join(" · ") }}
                </div>
              </button>

              <div v-if="filteredOrders.length === 0" class="p-10 text-center text-sm text-slate-500">
                {{
                  orderData.orders.length === 0
                    ? "No orders yet. Orders placed on Kiosk tablets will appear here automatically every 5 seconds."
                    : "No orders match the current filter."
                }}
              </div>
            </div>
          </div>

          <!-- Right: Order Detail -->
          <div class="lg:col-span-7 bg-white rounded-xl border border-slate-200 overflow-hidden flex flex-col max-h-[68vh]">
            <div v-if="!selectedOrder" class="flex-1 flex items-center justify-center p-12 text-sm text-slate-400">
              Select an order on the left to record payment or copy photos to a USB drive.
            </div>

            <div v-else class="overflow-y-auto flex-1 divide-y divide-slate-200">
              <!-- Order Header -->
              <div class="p-5 flex flex-wrap items-start justify-between gap-4">
                <div class="space-y-1">
                  <div class="flex items-center gap-2.5 flex-wrap">
                    <h2 class="text-2xl font-extrabold font-mono text-indigo-700">{{ selectedOrder.orderNumber }}</h2>
                    <span class="px-2.5 py-0.5 text-xs rounded-full font-semibold uppercase tracking-wide" :class="paymentStatusClass(selectedOrder.paymentStatus)">
                      {{ selectedOrder.paymentStatus }}
                    </span>
                  </div>
                  <div class="text-base font-semibold text-slate-900">{{ selectedOrder.customerName }}</div>
                  <div class="text-xs text-slate-500 flex flex-wrap gap-x-3 gap-y-1">
                    <span v-if="selectedOrder.customerEmail">{{ selectedOrder.customerEmail }}</span>
                    <span v-if="selectedOrder.customerPhone">{{ selectedOrder.customerPhone }}</span>
                    <span>Placed {{ formatTime(selectedOrder.createdAt) }}</span>
                  </div>
                  <div v-if="selectedOrder.shippingAddress" class="text-xs text-slate-600 pt-1">
                    <strong>Ship Prints To:</strong> {{ formatShippingAddress(selectedOrder.shippingAddress) }}
                  </div>
                </div>

                <div class="text-right space-y-2">
                  <div class="text-2xl font-extrabold font-mono text-slate-900">
                    {{ formatCents(selectedOrder.totalCents) }}
                  </div>
                  <button
                    @click="openQrModal"
                    class="px-3 py-1.5 text-xs font-semibold rounded-lg border border-indigo-200 bg-indigo-50 text-indigo-700 hover:bg-indigo-100"
                  >
                    Show Square POS QR
                  </button>
                </div>
              </div>

              <!-- Payment Reconciliation -->
              <div class="p-5 bg-slate-50/70 space-y-3">
                <template v-if="selectedOrder.paymentStatus === 'pending'">
                  <div class="text-xs font-semibold uppercase tracking-wide text-slate-500">Record Payment</div>
                  <div class="flex flex-wrap gap-1.5">
                    <button
                      v-for="m in PAYMENT_METHODS"
                      :key="m.id"
                      @click="paymentMethod = m.id"
                      class="px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors"
                      :class="paymentMethod === m.id ? 'bg-slate-900 text-white border-slate-900' : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-100'"
                    >
                      {{ m.label }}
                    </button>
                  </div>
                  <div class="flex flex-wrap items-center gap-2">
                    <input
                      v-model="paymentRefInput"
                      type="text"
                      placeholder="Receipt / Square ref # (optional)"
                      class="flex-1 min-w-[200px] border border-slate-300 bg-white rounded-lg px-3 py-2 text-sm"
                    />
                    <button
                      @click="submitPaymentStatus('paid')"
                      :disabled="isSavingPayment"
                      class="px-5 py-2 text-sm font-bold rounded-lg bg-emerald-600 text-white hover:bg-emerald-700 disabled:bg-slate-400 shadow-xs"
                    >
                      {{ isSavingPayment ? "Saving..." : `Mark Paid (${formatCents(selectedOrder.totalCents)})` }}
                    </button>
                  </div>
                </template>

                <template v-else-if="selectedOrder.paymentStatus === 'paid'">
                  <div class="flex flex-wrap items-center justify-between gap-3">
                    <div class="text-sm text-emerald-800">
                      <strong>Paid via {{ paymentMethodLabel(selectedOrder.paymentMethod) }}</strong>
                      <span v-if="selectedOrder.paymentReference" class="font-mono text-xs ml-2 px-1.5 py-0.5 rounded bg-emerald-100">
                        Ref: {{ selectedOrder.paymentReference }}
                      </span>
                      <div v-if="selectedOrder.paidAt" class="text-xs text-emerald-700/80 mt-0.5">
                        Recorded {{ formatTime(selectedOrder.paidAt) }}
                      </div>
                    </div>
                    <div class="flex items-center gap-2">
                      <button @click="submitPaymentStatus('pending')" class="px-2.5 py-1 text-xs rounded-md border border-slate-300 bg-white text-slate-600 hover:bg-slate-100">
                        Revert to Pending
                      </button>
                      <button @click="submitPaymentStatus('refunded')" class="px-2.5 py-1 text-xs rounded-md border border-rose-200 bg-rose-50 text-rose-700 hover:bg-rose-100">
                        Mark Refunded
                      </button>
                    </div>
                  </div>
                </template>

                <template v-else>
                  <div class="flex items-center justify-between gap-3">
                    <div class="text-sm font-semibold text-rose-700">This order has been marked Refunded.</div>
                    <button @click="submitPaymentStatus('paid')" class="px-3 py-1 text-xs rounded-md border border-slate-300 bg-white text-slate-700 hover:bg-slate-100">
                      Restore to Paid
                    </button>
                  </div>
                </template>
              </div>

              <!-- Instant USB-C/A Fulfillment -->
              <OrderUsbFulfillmentPanel
                v-if="hasDigitalPhotos(selectedOrder)"
                :order="selectedOrder"
                :has-pending-usb="hasPendingUsb(selectedOrder)"
                :usb-drives="usbDrives"
                @refresh-drives="refreshUsbDrives"
                @copied="onUsbCopied"
                @banner="bannerMessage = $event; errorMessage = null"
                @error="errorMessage = $event"
                @copying-change="isCopyingUsb = $event"
              />

              <!-- Line Items & Totals -->
              <div class="p-5 space-y-3">
                <h3 class="text-xs font-semibold uppercase tracking-wide text-slate-500">
                  Order Items &amp; Fulfillment
                </h3>
                <div class="divide-y divide-slate-100 border border-slate-200 rounded-xl overflow-hidden">
                  <div v-for="item in selectedOrder.items" :key="item.id" class="p-3 flex items-center justify-between gap-3 bg-white">
                    <div class="flex items-center gap-3 min-w-0">
                      <img
                        v-if="item.photoId && eventStore.lanStatus.running"
                        :src="`${thumbBaseUrl}/${item.photoId}`"
                        class="w-10 h-12 rounded object-cover bg-slate-200 flex-shrink-0"
                      />
                      <div class="min-w-0">
                        <div class="text-sm font-semibold text-slate-900">
                          {{ item.quantity > 1 ? `${item.quantity}× ` : "" }}{{ item.productName }}
                        </div>
                        <div class="text-xs text-slate-500 flex flex-wrap gap-x-2">
                          <span v-if="item.subjectDisplayName">{{ item.subjectDisplayName }}</span>
                          <span v-if="item.photoFilename" class="font-mono text-slate-400">{{ item.photoFilename }}</span>
                        </div>
                      </div>
                    </div>

                    <div class="flex items-center gap-3 flex-shrink-0">
                      <span class="font-mono text-sm font-semibold text-slate-800">{{ formatCents(item.lineTotalCents) }}</span>
                      <button
                        @click="toggleItemFulfillment(item)"
                        class="px-2.5 py-1 text-xs font-semibold rounded-full border transition-colors"
                        :class="item.fulfillmentStatus === 'fulfilled' ? 'bg-emerald-50 border-emerald-200 text-emerald-700 hover:bg-emerald-100' : 'bg-slate-50 border-slate-300 text-slate-600 hover:bg-slate-100'"
                      >
                        {{ item.fulfillmentStatus === "fulfilled" ? "Fulfilled ✓" : "Mark Fulfilled" }}
                      </button>
                    </div>
                  </div>
                </div>

                <div class="text-sm space-y-1 pt-2 max-w-xs ml-auto">
                  <div class="flex justify-between text-slate-500">
                    <span>Subtotal</span>
                    <span class="font-mono">{{ formatCents(selectedOrder.subtotalCents) }}</span>
                  </div>
                  <div v-if="selectedOrder.shippingCents > 0" class="flex justify-between text-slate-500">
                    <span>Shipping (S&amp;H · untaxed)</span>
                    <span class="font-mono">{{ formatCents(selectedOrder.shippingCents) }}</span>
                  </div>
                  <div class="flex justify-between text-slate-500">
                    <span>Tax ({{ (selectedOrder.taxRateBasisPoints / 100).toFixed(2) }}%)</span>
                    <span class="font-mono">{{ formatCents(selectedOrder.taxCents) }}</span>
                  </div>
                  <div class="flex justify-between font-bold text-slate-900 pt-1 border-t border-slate-200">
                    <span>Total</span>
                    <span class="font-mono">{{ formatCents(selectedOrder.totalCents) }}</span>
                  </div>
                </div>
              </div>

              <!-- Staff Notes -->
              <div class="p-5 space-y-2">
                <label class="text-xs font-semibold uppercase tracking-wide text-slate-500 block">Order Notes</label>
                <div class="flex gap-2">
                  <input
                    v-model="notesInput"
                    type="text"
                    placeholder="Add delivery notes, special requests, or accounting flags..."
                    class="flex-1 border border-slate-300 rounded-lg px-3 py-1.5 text-sm"
                  />
                  <button
                    @click="saveOrderNotes"
                    :disabled="isSavingNotes"
                    class="px-3.5 py-1.5 text-xs font-semibold rounded-lg border border-slate-300 bg-white text-slate-700 hover:bg-slate-50"
                  >
                    {{ isSavingNotes ? "Saving..." : "Save Note" }}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- ================= PRICING & TAX TAB ================= -->
      <PricingCatalogPanel
        v-show="subTab === 'pricing'"
        ref="pricingRef"
        @banner="bannerMessage = $event; errorMessage = null"
        @error="errorMessage = $event"
      />
    </template>

    <!-- ================= SQUARE POS QR MODAL ================= -->
    <div
      v-if="showQrModal && selectedOrder"
      class="fixed inset-0 z-50 bg-slate-900/60 backdrop-blur-xs flex items-center justify-center p-4"
      @click.self="showQrModal = false"
    >
      <div class="bg-white rounded-2xl shadow-2xl border border-slate-200 w-full max-w-sm p-6 text-center space-y-4">
        <div class="flex items-center justify-between">
          <span class="font-mono text-lg font-extrabold text-indigo-700">{{ selectedOrder.orderNumber }}</span>
          <button @click="showQrModal = false" class="text-slate-400 hover:text-slate-600 text-2xl leading-none">&times;</button>
        </div>
        <div class="bg-slate-50 rounded-2xl border border-slate-200 p-4 inline-flex items-center justify-center">
          <img v-if="qrDataUrl" :src="qrDataUrl" alt="Square POS QR" class="w-52 h-52 block" />
        </div>
        <div>
          <div class="text-sm font-semibold text-slate-800">{{ selectedOrder.customerName }}</div>
          <div class="text-2xl font-extrabold font-mono text-slate-900 mt-0.5">
            {{ formatCents(selectedOrder.totalCents) }}
          </div>
          <p class="text-xs text-slate-500 mt-1">
            Scan with staff iPhone to launch Square POS pre-filled with this exact amount &amp; order note.
          </p>
        </div>
        <button @click="showQrModal = false" class="w-full py-2.5 rounded-xl bg-slate-900 text-white text-sm font-semibold hover:bg-slate-800">
          Close
        </button>
      </div>
    </div>
  </div>
</template>
