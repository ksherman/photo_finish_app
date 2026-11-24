<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { Channel, invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import QRCode from "qrcode";
import { useEventStore } from "@/stores/event";
import type {
  CsvExportResult,
  EventSummary,
  OrderItemRecord,
  OrderListResponse,
  OrderRecord,
  ProductRecord,
  UsbCopyProgressEvent,
  UsbCopyResult,
  UsbDriveInfo,
  UsbOrderPlan,
} from "@/types";

const eventStore = useEventStore();

type SubTab = "desk" | "pricing";
type QueueFilter = "all" | "pending_payment" | "needs_usb" | "paid" | "pending_fulfillment" | "refunded";

const subTab = ref<SubTab>("desk");
const queueFilter = ref<QueueFilter>("all");
const searchQuery = ref("");

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
    taxPaidCents: 0,
    pendingRevenueCents: 0,
  },
  orders: [],
});

const selectedOrderId = ref<string | null>(null);
const bannerMessage = ref<string | null>(null);
const errorMessage = ref<string | null>(null);

// Payment action state
const paymentMethod = ref<string>("square_qr");
const paymentRefInput = ref("");
const notesInput = ref("");
const isSavingPayment = ref(false);
const isSavingNotes = ref(false);
const showQrModal = ref(false);
const qrDataUrl = ref("");

// USB Fulfillment state
const usbPlan = ref<UsbOrderPlan | null>(null);
const loadingUsbPlan = ref(false);
const usbDrives = ref<UsbDriveInfo[]>([]);
const selectedUsbMount = ref<string>("");
const selectedUsbDiskId = ref<string | null>(null);
const autoEjectUsb = ref(true);
const autoMarkUsbFulfilled = ref(true);
const isCopyingUsb = ref(false);
const usbProgress = ref<UsbCopyProgressEvent | null>(null);

// Pricing & Settings state
const products = ref<ProductRecord[]>([]);
const editingPrices = ref<Record<string, string>>({});
const editingNames = ref<Record<string, string>>({});
const orderCodeInput = ref("");
const stationCodeInput = ref("");
const taxRatePercentInput = ref("8.50");
const isSavingSettings = ref(false);
const showNewProductForm = ref(false);
const newProdType = ref("print");
const newProdName = ref("");
const newProdSize = ref("");
const newProdPrice = ref("25.00");

let ordersPollTimer: ReturnType<typeof setInterval> | null = null;
let usbPollTimer: ReturnType<typeof setInterval> | null = null;

const thumbBaseUrl = computed(() => {
  const port = eventStore.lanStatus.port || 4000;
  return `http://127.0.0.1:${port}/photos/thumbnail`;
});

function formatCents(cents: number): string {
  const sign = cents < 0 ? "-" : "";
  const abs = Math.abs(cents);
  return `${sign}$${(abs / 100).toFixed(2)}`;
}

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let val = bytes;
  let i = 0;
  while (val >= 1024 && i < units.length - 1) {
    val /= 1024;
    i += 1;
  }
  return `${val.toFixed(i >= 2 ? 1 : 0)} ${units[i]}`;
}

function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

function paymentMethodLabel(method: string | null): string {
  switch (method) {
    case "square_qr":
      return "Square QR";
    case "square_terminal":
      return "Square Terminal";
    case "cash":
      return "Cash";
    case "check":
      return "Check";
    case "other":
      return "Other";
    default:
      return method || "—";
  }
}

function hasUsbAddon(order: OrderRecord): boolean {
  return order.items.some((i) => i.productType === "usb_addon");
}

function hasPendingUsb(order: OrderRecord): boolean {
  return (
    order.paymentStatus !== "refunded" &&
    order.items.some((i) => i.productType === "usb_addon" && i.fulfillmentStatus !== "fulfilled")
  );
}

function hasPrints(order: OrderRecord): boolean {
  return order.items.some((i) => i.productType === "print");
}

function hasDigitalPhotos(order: OrderRecord): boolean {
  return order.items.some((i) =>
    ["full_digital", "single_digital", "usb_addon"].includes(i.productType)
  );
}

function isFullyFulfilled(order: OrderRecord): boolean {
  return order.items.length > 0 && order.items.every((i) => i.fulfillmentStatus === "fulfilled");
}

const filteredOrders = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  return orderData.value.orders.filter((ord) => {
    if (queueFilter.value === "pending_payment" && ord.paymentStatus !== "pending") return false;
    if (queueFilter.value === "paid" && ord.paymentStatus !== "paid") return false;
    if (queueFilter.value === "refunded" && ord.paymentStatus !== "refunded") return false;
    if (queueFilter.value === "needs_usb" && !hasPendingUsb(ord)) return false;
    if (
      queueFilter.value === "pending_fulfillment" &&
      (ord.paymentStatus === "refunded" || isFullyFulfilled(ord))
    ) {
      return false;
    }

    if (!q) return true;
    if (ord.orderNumber.toLowerCase().includes(q)) return true;
    if (ord.customerName.toLowerCase().includes(q)) return true;
    if ((ord.customerEmail || "").toLowerCase().includes(q)) return true;
    if ((ord.customerPhone || "").toLowerCase().includes(q)) return true;
    if (
      ord.items.some(
        (i) =>
          (i.subjectDisplayName || "").toLowerCase().includes(q) ||
          i.productName.toLowerCase().includes(q)
      )
    ) {
      return true;
    }
    return false;
  });
});

const selectedOrder = computed<OrderRecord | null>(() => {
  if (!selectedOrderId.value) return null;
  return orderData.value.orders.find((o) => o.id === selectedOrderId.value) || null;
});

const selectedUsbDriveObj = computed(() =>
  usbDrives.value.find((d) => d.mountPoint === selectedUsbMount.value) || null
);

const usbInsufficientSpace = computed(() => {
  if (!selectedUsbDriveObj.value || !usbPlan.value) return false;
  if (selectedUsbDriveObj.value.freeBytes <= 0) return false;
  return usbPlan.value.totalBytes > selectedUsbDriveObj.value.freeBytes;
});

async function loadOrders(silent = false) {
  if (!eventStore.activeEvent) return;
  try {
    const res = await invoke<OrderListResponse>("list_orders");
    const prevOrderCount = orderData.value.orders.length;
    orderData.value = res;
    if (res.orders.length !== prevOrderCount) {
      eventStore.refreshActiveEvent();
    }
    if (
      !selectedOrderId.value ||
      !res.orders.some((o) => o.id === selectedOrderId.value)
    ) {
      selectedOrderId.value = res.orders[0]?.id || null;
    }
  } catch (err) {
    if (!silent) errorMessage.value = String(err);
  }
}

async function loadUsbPlanForSelected() {
  const ord = selectedOrder.value;
  if (!ord || !hasDigitalPhotos(ord)) {
    usbPlan.value = null;
    return;
  }
  loadingUsbPlan.value = true;
  try {
    usbPlan.value = await invoke<UsbOrderPlan>("get_order_usb_plan", {
      orderId: ord.id,
    });
  } catch (err) {
    usbPlan.value = null;
    errorMessage.value = String(err);
  } finally {
    loadingUsbPlan.value = false;
  }
}

async function refreshUsbDrives() {
  try {
    const drives = await invoke<UsbDriveInfo[]>("discover_usb_drives");
    usbDrives.value = drives;
    // Auto-select first non-DCIM USB drive if nothing custom is selected
    if (!selectedUsbMount.value) {
      const preferred = drives.find((d) => !d.hasDcim) || drives[0];
      if (preferred) {
        selectedUsbMount.value = preferred.mountPoint;
        selectedUsbDiskId.value = preferred.diskId;
      }
    } else {
      const match = drives.find((d) => d.mountPoint === selectedUsbMount.value);
      if (match) {
        selectedUsbDiskId.value = match.diskId;
      }
    }
  } catch {
    // Non-fatal on unsupported OS
  }
}

function selectUsbDrive(drive: UsbDriveInfo) {
  selectedUsbMount.value = drive.mountPoint;
  selectedUsbDiskId.value = drive.diskId;
}

async function pickCustomUsbFolder() {
  const picked = await open({
    directory: true,
    multiple: false,
    title: "Select Destination Folder or USB Drive",
  });
  if (typeof picked === "string" && picked) {
    selectedUsbMount.value = picked;
    const match = usbDrives.value.find((d) => d.mountPoint === picked);
    selectedUsbDiskId.value = match ? match.diskId : null;
  }
}

async function startUsbCopy() {
  const ord = selectedOrder.value;
  if (!ord || !selectedUsbMount.value || isCopyingUsb.value) return;
  isCopyingUsb.value = true;
  usbProgress.value = null;
  errorMessage.value = null;
  bannerMessage.value = null;

  try {
    const channel = new Channel<UsbCopyProgressEvent>();
    channel.onmessage = (ev) => {
      usbProgress.value = ev;
    };

    const res = await invoke<UsbCopyResult>("copy_order_to_usb", {
      request: {
        orderId: ord.id,
        destinationDir: selectedUsbMount.value,
        ejectDiskId: autoEjectUsb.value ? selectedUsbDiskId.value : null,
        autoMarkFulfilled: autoMarkUsbFulfilled.value,
      },
      onProgress: channel,
    });

    const speed = `${res.mbPerSec.toFixed(1)} MB/s`;
    const ejectNote = res.ejected
      ? " — drive safely ejected!"
      : res.ejectWarning
        ? ` (copy succeeded, eject warning: ${res.ejectWarning})`
        : "";
    bannerMessage.value = `Copied ${res.filesCopied} photo(s) (${formatBytes(res.bytesCopied)}) for ${ord.orderNumber} in ${res.elapsedSecs.toFixed(1)}s (${speed})${ejectNote}`;

    if (res.ejected) {
      selectedUsbMount.value = "";
      selectedUsbDiskId.value = null;
    }
    await Promise.all([loadOrders(true), refreshUsbDrives()]);
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isCopyingUsb.value = false;
    usbProgress.value = null;
  }
}

async function handleManualEject(drive: UsbDriveInfo) {
  errorMessage.value = null;
  try {
    await invoke("eject_usb_drive", { diskId: drive.diskId });
    bannerMessage.value = `Safely ejected "${drive.volumeName}".`;
    if (selectedUsbMount.value === drive.mountPoint) {
      selectedUsbMount.value = "";
      selectedUsbDiskId.value = null;
    }
    await refreshUsbDrives();
  } catch (err) {
    errorMessage.value = String(err);
  }
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
  const nextStatus = item.fulfillmentStatus === "fulfilled" ? "pending" : "fulfilled";
  errorMessage.value = null;
  try {
    await invoke<OrderRecord>("update_order_item_fulfillment", {
      orderId: ord.id,
      itemId: item.id,
      fulfillmentStatus: nextStatus,
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
  const slug = allLibraryEvents
    ? "photofinish-all-events-orders"
    : `${eventStore.activeEvent?.slug || "event"}-orders`;
  const targetPath = await save({
    defaultPath: `${slug}.csv`,
    filters: [{ name: "CSV Spreadsheet", extensions: ["csv"] }],
    title: allLibraryEvents
      ? "Export Consolidated Orders CSV (All Events)"
      : "Export Event Orders CSV",
  });
  if (!targetPath) return;

  try {
    const res = await invoke<CsvExportResult>("export_orders_csv", {
      outputPath: targetPath,
      allLibraryEvents,
    });
    const skippedNote =
      res.skippedEvents.length > 0
        ? ` (Skipped ${res.skippedEvents.length}: ${res.skippedEvents.join(", ")})`
        : "";
    bannerMessage.value = `Exported ${res.orderCount} order(s) (${res.itemRowCount} line items across ${res.eventCount} event${res.eventCount === 1 ? "" : "s"}) to ${res.outputPath}.${skippedNote}`;
  } catch (err) {
    errorMessage.value = String(err);
  }
}

// Pricing & Settings helpers
async function loadPricingCatalog() {
  if (!eventStore.activeEvent) return;
  try {
    const list = await invoke<ProductRecord[]>("list_event_products");
    products.value = list;
    const priceMap: Record<string, string> = {};
    const nameMap: Record<string, string> = {};
    for (const p of list) {
      priceMap[p.id] = (p.priceCents / 100).toFixed(2);
      nameMap[p.id] = p.name;
    }
    editingPrices.value = priceMap;
    editingNames.value = nameMap;
    orderCodeInput.value = eventStore.activeEvent.orderCode;
    stationCodeInput.value = eventStore.activeEvent.stationCode;
    taxRatePercentInput.value = (eventStore.activeEvent.taxRateBasisPoints / 100).toFixed(2);
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function saveProductRow(p: ProductRecord, overrideAvailable?: boolean) {
  errorMessage.value = null;
  const dollars = parseFloat(editingPrices.value[p.id] ?? "");
  if (Number.isNaN(dollars) || dollars < 0) {
    errorMessage.value = `Invalid price for "${p.name}"`;
    return;
  }
  const priceCents = Math.round(dollars * 100);
  try {
    await invoke<ProductRecord>("upsert_event_product", {
      input: {
        id: p.id,
        productType: p.productType,
        name: (editingNames.value[p.id] || p.name).trim(),
        size: p.size,
        priceCents,
        isAvailable: overrideAvailable !== undefined ? overrideAvailable : p.isAvailable,
        displayOrder: p.displayOrder,
      },
    });
    bannerMessage.value = `Updated "${editingNames.value[p.id] || p.name}" (${formatCents(priceCents)}). Kiosks will reflect this immediately.`;
    await loadPricingCatalog();
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function addNewProduct() {
  const dollars = parseFloat(newProdPrice.value);
  if (!newProdName.value.trim() || Number.isNaN(dollars) || dollars < 0) {
    errorMessage.value = "Provide a valid product name and price.";
    return;
  }
  errorMessage.value = null;
  try {
    await invoke<ProductRecord>("upsert_event_product", {
      input: {
        id: null,
        productType: newProdType.value,
        name: newProdName.value.trim(),
        size: newProdSize.value.trim() || null,
        priceCents: Math.round(dollars * 100),
        isAvailable: true,
        displayOrder: products.value.length,
      },
    });
    newProdName.value = "";
    newProdSize.value = "";
    newProdPrice.value = "25.00";
    showNewProductForm.value = false;
    bannerMessage.value = "Added new product to event catalog.";
    await loadPricingCatalog();
  } catch (err) {
    errorMessage.value = String(err);
  }
}

async function saveEventOrderSettings() {
  if (isSavingSettings.value) return;
  const pct = parseFloat(taxRatePercentInput.value);
  if (Number.isNaN(pct) || pct < 0 || pct > 25) {
    errorMessage.value = "Tax rate must be between 0% and 25%.";
    return;
  }
  isSavingSettings.value = true;
  errorMessage.value = null;
  try {
    const updated = await invoke<EventSummary>("update_event_order_settings", {
      input: {
        orderCode: orderCodeInput.value.trim(),
        stationCode: stationCodeInput.value.trim(),
        taxRateBasisPoints: Math.round(pct * 100),
      },
    });
    eventStore.activeEvent = updated;
    bannerMessage.value = `Updated event order settings (${updated.orderCode}-${updated.stationCode} · ${(updated.taxRateBasisPoints / 100).toFixed(2)}% tax).`;
  } catch (err) {
    errorMessage.value = String(err);
  } finally {
    isSavingSettings.value = false;
  }
}

// Sync detail form fields & USB plan whenever selectedOrder changes
watch(
  selectedOrder,
  (ord, prev) => {
    if (!ord) {
      usbPlan.value = null;
      return;
    }
    if (!prev || ord.id !== prev.id) {
      paymentMethod.value = ord.paymentMethod || "square_qr";
      paymentRefInput.value = ord.paymentReference || "";
      notesInput.value = ord.notes || "";
      loadUsbPlanForSelected();
    }
  },
  { immediate: true }
);

watch(
  () => eventStore.activeEvent?.id,
  () => {
    selectedOrderId.value = null;
    if (eventStore.activeEvent) {
      loadOrders();
      loadPricingCatalog();
    }
  }
);

onMounted(() => {
  if (eventStore.activeEvent) {
    loadOrders();
    loadPricingCatalog();
  }
  refreshUsbDrives();
  ordersPollTimer = setInterval(() => {
    if (eventStore.activeEvent && subTab.value === "desk") {
      loadOrders(true);
    }
  }, 5000);
  usbPollTimer = setInterval(() => {
    if (subTab.value === "desk" && !isCopyingUsb.value) {
      refreshUsbDrives();
    }
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
              @click="subTab = 'pricing'; loadPricingCatalog()"
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

      <div v-if="bannerMessage" class="p-3.5 rounded-lg bg-emerald-50 border border-emerald-200 text-sm text-emerald-800 flex items-center justify-between">
        <span>{{ bannerMessage }}</span>
        <button @click="bannerMessage = null" class="text-emerald-600 font-bold ml-4">&times;</button>
      </div>
      <div v-if="errorMessage" class="p-3.5 rounded-lg bg-red-50 border border-red-200 text-sm text-red-700 flex items-center justify-between">
        <span>{{ errorMessage }}</span>
        <button @click="errorMessage = null" class="text-red-600 font-bold ml-4">&times;</button>
      </div>

      <!-- ================= ORDER DESK TAB ================= -->
      <div v-show="subTab === 'desk'" class="space-y-4">
        <!-- KPI Summary Cards -->
        <div class="grid grid-cols-2 lg:grid-cols-4 gap-3">
          <button
            @click="queueFilter = queueFilter === 'pending_payment' ? 'all' : 'pending_payment'"
            class="text-left p-4 rounded-xl border transition-all"
            :class="
              queueFilter === 'pending_payment'
                ? 'border-amber-400 bg-amber-50 ring-2 ring-amber-200'
                : 'border-slate-200 bg-white hover:border-slate-300'
            "
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
            :class="
              queueFilter === 'needs_usb'
                ? 'border-indigo-400 bg-indigo-50 ring-2 ring-indigo-200'
                : 'border-slate-200 bg-white hover:border-slate-300'
            "
          >
            <div class="text-xs font-medium text-slate-500 uppercase tracking-wide">Needs USB Copy</div>
            <div class="mt-1 flex items-baseline justify-between">
              <span class="text-2xl font-bold" :class="orderData.stats.pendingUsbCount > 0 ? 'text-indigo-700' : 'text-slate-900'">
                {{ orderData.stats.pendingUsbCount }}
              </span>
              <span class="text-xs text-slate-500">
                {{ usbDrives.filter((d) => !d.hasDcim).length }} USB drive{{ usbDrives.filter((d) => !d.hasDcim).length === 1 ? '' : 's' }} ready
              </span>
            </div>
          </button>

          <button
            @click="queueFilter = queueFilter === 'paid' ? 'all' : 'paid'"
            class="text-left p-4 rounded-xl border transition-all"
            :class="
              queueFilter === 'paid'
                ? 'border-emerald-400 bg-emerald-50 ring-2 ring-emerald-200'
                : 'border-slate-200 bg-white hover:border-slate-300'
            "
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
            :class="
              queueFilter === 'all'
                ? 'border-slate-400 bg-slate-50 ring-2 ring-slate-200'
                : 'border-slate-200 bg-white hover:border-slate-300'
            "
          >
            <div class="text-xs font-medium text-slate-500 uppercase tracking-wide">Total Orders</div>
            <div class="mt-1 flex items-baseline justify-between">
              <span class="text-2xl font-bold text-slate-900">{{ orderData.stats.totalOrders }}</span>
              <span class="text-xs text-slate-500">
                {{ orderData.stats.pendingFulfillmentCount }} open
              </span>
            </div>
          </button>
        </div>

        <!-- Two-Pane Order Desk -->
        <div class="grid grid-cols-1 lg:grid-cols-12 gap-5">
          <!-- Left: Order Queue (5 cols) -->
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
                  v-for="f in [
                    { id: 'all', label: 'All' },
                    { id: 'pending_payment', label: 'Pending Pay' },
                    { id: 'needs_usb', label: 'Needs USB' },
                    { id: 'paid', label: 'Paid' },
                    { id: 'pending_fulfillment', label: 'Open Items' },
                    { id: 'refunded', label: 'Refunded' },
                  ] as const"
                  :key="f.id"
                  @click="queueFilter = f.id"
                  class="px-2.5 py-1 rounded-full font-medium transition-colors"
                  :class="
                    queueFilter === f.id
                      ? 'bg-slate-900 text-white'
                      : 'bg-white border border-slate-200 text-slate-600 hover:bg-slate-100'
                  "
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
                :class="
                  selectedOrderId === ord.id
                    ? 'bg-blue-50/80 border-l-4 border-l-blue-600'
                    : 'hover:bg-slate-50 border-l-4 border-l-transparent'
                "
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
                    <span
                      class="px-2 py-0.5 rounded-full font-semibold"
                      :class="{
                        'bg-amber-100 text-amber-800': ord.paymentStatus === 'pending',
                        'bg-emerald-100 text-emerald-800': ord.paymentStatus === 'paid',
                        'bg-rose-100 text-rose-700': ord.paymentStatus === 'refunded',
                      }"
                    >
                      {{ ord.paymentStatus === 'pending' ? 'Pending Pay' : ord.paymentStatus === 'paid' ? 'Paid' : 'Refunded' }}
                    </span>
                    <span
                      v-if="hasPendingUsb(ord)"
                      class="px-2 py-0.5 rounded-full bg-indigo-100 text-indigo-800 font-semibold"
                    >
                      USB Needed
                    </span>
                    <span
                      v-else-if="hasUsbAddon(ord)"
                      class="px-2 py-0.5 rounded-full bg-slate-100 text-slate-600"
                    >
                      USB Done ✓
                    </span>
                    <span
                      v-if="hasPrints(ord)"
                      class="px-2 py-0.5 rounded-full bg-purple-100 text-purple-800 font-medium"
                    >
                      Prints
                    </span>
                  </div>
                  <span class="text-slate-400 flex-shrink-0">{{ formatTime(ord.createdAt) }}</span>
                </div>

                <div class="text-xs text-slate-500 truncate">
                  {{
                    ord.items
                      .map((i) => `${i.productName}${i.subjectDisplayName ? ` (${i.subjectDisplayName})` : ''}`)
                      .join(" · ")
                  }}
                </div>
              </button>

              <div v-if="filteredOrders.length === 0" class="p-10 text-center text-sm text-slate-500">
                <template v-if="orderData.orders.length === 0">
                  No orders yet. Orders placed on Kiosk tablets will appear here automatically every 5 seconds.
                </template>
                <template v-else>
                  No orders match the current filter.
                </template>
              </div>
            </div>
          </div>

          <!-- Right: Order Detail, Payment & Instant USB Fulfillment (7 cols) -->
          <div class="lg:col-span-7 bg-white rounded-xl border border-slate-200 overflow-hidden flex flex-col max-h-[68vh]">
            <div v-if="!selectedOrder" class="flex-1 flex items-center justify-center p-12 text-sm text-slate-400">
              Select an order on the left to record payment or copy photos to a USB drive.
            </div>

            <div v-else class="overflow-y-auto flex-1 divide-y divide-slate-200">
              <!-- Order Header -->
              <div class="p-5 flex flex-wrap items-start justify-between gap-4">
                <div class="space-y-1">
                  <div class="flex items-center gap-2.5 flex-wrap">
                    <h2 class="text-2xl font-extrabold font-mono text-indigo-700">
                      {{ selectedOrder.orderNumber }}
                    </h2>
                    <span
                      class="px-2.5 py-0.5 text-xs rounded-full font-semibold uppercase tracking-wide"
                      :class="{
                        'bg-amber-100 text-amber-800': selectedOrder.paymentStatus === 'pending',
                        'bg-emerald-100 text-emerald-800': selectedOrder.paymentStatus === 'paid',
                        'bg-rose-100 text-rose-700': selectedOrder.paymentStatus === 'refunded',
                      }"
                    >
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
                    <strong>Ship Prints To:</strong>
                    {{ selectedOrder.shippingAddress.street }}
                    <span v-if="selectedOrder.shippingAddress.street2"> {{ selectedOrder.shippingAddress.street2 }}</span>,
                    {{ selectedOrder.shippingAddress.city }}, {{ selectedOrder.shippingAddress.state }}
                    {{ selectedOrder.shippingAddress.zip }}
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

              <!-- Payment Reconciliation Box -->
              <div class="p-5 bg-slate-50/70 space-y-3">
                <template v-if="selectedOrder.paymentStatus === 'pending'">
                  <div class="text-xs font-semibold uppercase tracking-wide text-slate-500">
                    Record Payment
                  </div>
                  <div class="flex flex-wrap gap-1.5">
                    <button
                      v-for="m in [
                        { id: 'square_qr', label: 'Square QR' },
                        { id: 'square_terminal', label: 'Square Terminal' },
                        { id: 'cash', label: 'Cash' },
                        { id: 'check', label: 'Check' },
                        { id: 'other', label: 'Other' },
                      ]"
                      :key="m.id"
                      @click="paymentMethod = m.id"
                      class="px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors"
                      :class="
                        paymentMethod === m.id
                          ? 'bg-slate-900 text-white border-slate-900'
                          : 'bg-white text-slate-700 border-slate-300 hover:bg-slate-100'
                      "
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
                      <button
                        @click="submitPaymentStatus('pending')"
                        class="px-2.5 py-1 text-xs rounded-md border border-slate-300 bg-white text-slate-600 hover:bg-slate-100"
                      >
                        Revert to Pending
                      </button>
                      <button
                        @click="submitPaymentStatus('refunded')"
                        class="px-2.5 py-1 text-xs rounded-md border border-rose-200 bg-rose-50 text-rose-700 hover:bg-rose-100"
                      >
                        Mark Refunded
                      </button>
                    </div>
                  </div>
                </template>

                <template v-else>
                  <div class="flex items-center justify-between gap-3">
                    <div class="text-sm font-semibold text-rose-700">This order has been marked Refunded.</div>
                    <button
                      @click="submitPaymentStatus('paid')"
                      class="px-3 py-1 text-xs rounded-md border border-slate-300 bg-white text-slate-700 hover:bg-slate-100"
                    >
                      Restore to Paid
                    </button>
                  </div>
                </template>
              </div>

              <!-- Instant USB-C/A Fulfillment Panel -->
              <div v-if="hasDigitalPhotos(selectedOrder)" class="p-5 space-y-3.5" :class="hasPendingUsb(selectedOrder) ? 'bg-indigo-50/40' : ''">
                <div class="flex items-center justify-between gap-2">
                  <div>
                    <h3 class="text-sm font-bold text-slate-900 flex items-center gap-2">
                      <span>Instant Dual USB-C/A Fulfillment</span>
                      <span
                        v-if="hasPendingUsb(selectedOrder)"
                        class="px-2 py-0.5 text-[11px] rounded-full bg-indigo-600 text-white font-semibold"
                      >
                        USB Ordered
                      </span>
                    </h3>
                    <p class="text-xs text-slate-500 mt-0.5">
                      Copies full-resolution original JPEGs sorted into <code>Subject / Apparatus / IMG.jpg</code> folders.
                    </p>
                  </div>
                  <button
                    @click="refreshUsbDrives"
                    class="px-2.5 py-1 text-xs font-medium rounded-md border border-slate-300 bg-white text-slate-600 hover:bg-slate-100"
                  >
                    Rescan USB
                  </button>
                </div>

                <!-- Resolved USB File Plan -->
                <div v-if="loadingUsbPlan" class="text-xs text-slate-500">Calculating order photos...</div>
                <div v-else-if="usbPlan" class="rounded-lg border border-slate-200 bg-white p-3 space-y-2 text-xs">
                  <div class="flex items-center justify-between font-semibold text-slate-800">
                    <span>{{ usbPlan.totalPhotos.toLocaleString() }} full-res photo(s) ready to copy</span>
                    <span class="font-mono text-indigo-700">{{ formatBytes(usbPlan.totalBytes) }}</span>
                  </div>
                  <div v-for="subj in usbPlan.subjects" :key="subj.subjectLabel" class="pl-2 border-l-2 border-indigo-200 space-y-0.5">
                    <div class="font-medium text-slate-800">
                      {{ subj.subjectLabel }} — {{ subj.photoCount }} photos ({{ formatBytes(subj.totalBytes) }})
                    </div>
                    <div class="text-slate-500 flex flex-wrap gap-x-2.5">
                      <span v-for="[sec, cnt] in subj.sections" :key="sec">{{ sec }}: {{ cnt }}</span>
                    </div>
                  </div>
                  <div v-if="usbPlan.missingFiles > 0" class="text-amber-700 font-medium">
                    Warning: {{ usbPlan.missingFiles }} file(s) missing on disk.
                  </div>
                </div>

                <!-- Detected External USB Drives -->
                <div class="space-y-2">
                  <div class="flex items-center justify-between">
                    <span class="text-xs font-semibold text-slate-600">Target USB Drive / Folder</span>
                    <button
                      @click="pickCustomUsbFolder"
                      class="text-xs font-medium text-blue-600 hover:underline"
                    >
                      Choose Custom Folder...
                    </button>
                  </div>

                  <div v-if="usbDrives.length > 0" class="grid grid-cols-1 sm:grid-cols-2 gap-2">
                    <div
                      v-for="drive in usbDrives"
                      :key="drive.mountPoint"
                      @click="selectUsbDrive(drive)"
                      class="p-2.5 rounded-lg border cursor-pointer transition-all flex items-center justify-between gap-2"
                      :class="
                        selectedUsbMount === drive.mountPoint
                          ? 'border-indigo-500 bg-indigo-50 ring-2 ring-indigo-200'
                          : 'border-slate-200 bg-white hover:border-slate-300'
                      "
                    >
                      <div class="min-w-0">
                        <div class="text-xs font-bold text-slate-900 flex items-center gap-1.5 truncate">
                          <span>{{ drive.volumeName }}</span>
                          <span
                            v-if="drive.hasDcim"
                            class="px-1.5 py-0.2 rounded bg-amber-100 text-amber-800 text-[10px] font-semibold"
                            title="This volume contains a DCIM folder — verify it is not a photographer's SD card!"
                          >
                            DCIM!
                          </span>
                        </div>
                        <div class="text-[11px] text-slate-500 font-mono truncate">
                          {{ drive.freeBytes > 0 ? `${formatBytes(drive.freeBytes)} free` : drive.mountPoint }}
                        </div>
                      </div>
                      <button
                        @click.stop="handleManualEject(drive)"
                        title="Safely eject drive"
                        class="px-2 py-1 text-[11px] rounded border border-slate-200 bg-white text-slate-600 hover:bg-slate-100 flex-shrink-0"
                      >
                        Eject
                      </button>
                    </div>
                  </div>
                  <div v-else class="text-xs text-slate-500 bg-white border border-dashed border-slate-300 rounded-lg p-3">
                    Plug in a Dual USB-C/A drive (detected automatically) or click <strong>Choose Custom Folder...</strong> above.
                  </div>

                  <div
                    v-if="selectedUsbMount && !selectedUsbDriveObj"
                    class="text-xs font-mono text-slate-600 bg-slate-100 rounded-md px-2.5 py-1.5 truncate"
                  >
                    Custom folder: {{ selectedUsbMount }}
                  </div>

                  <div v-if="selectedUsbDriveObj?.hasDcim" class="p-2.5 rounded-lg bg-amber-50 border border-amber-300 text-xs text-amber-900">
                    <strong>Caution:</strong> <code>{{ selectedUsbDriveObj.volumeName }}</code> has a <code>DCIM/</code> folder and may be a camera SD card. Double-check before copying!
                  </div>
                  <div v-if="usbInsufficientSpace" class="p-2.5 rounded-lg bg-red-50 border border-red-200 text-xs text-red-700">
                    Not enough free space on <strong>{{ selectedUsbDriveObj?.volumeName }}</strong> (needs {{ formatBytes(usbPlan?.totalBytes || 0) }}, only {{ formatBytes(selectedUsbDriveObj?.freeBytes || 0) }} free).
                  </div>
                </div>

                <!-- Live USB Copy Progress Bar -->
                <div v-if="isCopyingUsb && usbProgress" class="space-y-1.5">
                  <div class="flex justify-between text-xs font-medium text-indigo-800">
                    <span>
                      Copying {{ usbProgress.copiedFiles }} / {{ usbProgress.totalFiles }} ({{ usbProgress.currentFile }})
                    </span>
                    <span class="font-mono">
                      {{ usbProgress.mbPerSec.toFixed(1) }} MB/s · {{ Math.round(usbProgress.percentage) }}%
                    </span>
                  </div>
                  <div class="w-full h-2.5 bg-indigo-100 rounded-full overflow-hidden">
                    <div
                      class="h-full bg-indigo-600 transition-all duration-150"
                      :style="{ width: `${usbProgress.percentage}%` }"
                    />
                  </div>
                </div>

                <!-- Copy Controls & Primary Button -->
                <div class="flex flex-wrap items-center justify-between gap-3 pt-1">
                  <div class="flex flex-wrap items-center gap-4 text-xs text-slate-600">
                    <label v-if="selectedUsbDiskId" class="flex items-center gap-1.5 cursor-pointer">
                      <input v-model="autoEjectUsb" type="checkbox" class="rounded" />
                      <span>Auto-eject when done</span>
                    </label>
                    <label class="flex items-center gap-1.5 cursor-pointer">
                      <input v-model="autoMarkUsbFulfilled" type="checkbox" class="rounded" />
                      <span>Mark USB item fulfilled</span>
                    </label>
                  </div>

                  <button
                    @click="startUsbCopy"
                    :disabled="!selectedUsbMount || !usbPlan || usbPlan.totalPhotos === 0 || isCopyingUsb || usbInsufficientSpace"
                    class="px-5 py-2.5 text-sm font-bold rounded-lg bg-indigo-600 text-white hover:bg-indigo-700 disabled:bg-slate-300 disabled:text-slate-500 shadow-xs"
                  >
                    {{
                      isCopyingUsb
                        ? "Copying to USB..."
                        : `Copy ${usbPlan?.totalPhotos || 0} Photos${selectedUsbDriveObj ? ` to ${selectedUsbDriveObj.volumeName}` : ""}${autoEjectUsb && selectedUsbDiskId ? " & Eject" : ""}`
                    }}
                  </button>
                </div>
              </div>

              <!-- Order Line Items & Fulfillment Checklist -->
              <div class="p-5 space-y-3">
                <h3 class="text-xs font-semibold uppercase tracking-wide text-slate-500">
                  Order Items &amp; Fulfillment
                </h3>
                <div class="divide-y divide-slate-100 border border-slate-200 rounded-xl overflow-hidden">
                  <div
                    v-for="item in selectedOrder.items"
                    :key="item.id"
                    class="p-3 flex items-center justify-between gap-3 bg-white"
                  >
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
                      <span class="font-mono text-sm font-semibold text-slate-800">
                        {{ formatCents(item.lineTotalCents) }}
                      </span>
                      <button
                        @click="toggleItemFulfillment(item)"
                        class="px-2.5 py-1 text-xs font-semibold rounded-full border transition-colors"
                        :class="
                          item.fulfillmentStatus === 'fulfilled'
                            ? 'bg-emerald-50 border-emerald-200 text-emerald-700 hover:bg-emerald-100'
                            : 'bg-slate-50 border-slate-300 text-slate-600 hover:bg-slate-100'
                        "
                      >
                        {{ item.fulfillmentStatus === 'fulfilled' ? 'Fulfilled ✓' : 'Mark Fulfilled' }}
                      </button>
                    </div>
                  </div>
                </div>

                <!-- Totals Breakdown -->
                <div class="text-sm space-y-1 pt-2 max-w-xs ml-auto">
                  <div class="flex justify-between text-slate-500">
                    <span>Subtotal</span>
                    <span class="font-mono">{{ formatCents(selectedOrder.subtotalCents) }}</span>
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
                <label class="text-xs font-semibold uppercase tracking-wide text-slate-500 block">
                  Order Notes
                </label>
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
      <div v-show="subTab === 'pricing'" class="space-y-5">
        <!-- Event Order Code & Tax Settings -->
        <div class="bg-white rounded-xl border border-slate-200 p-5 space-y-3">
          <div>
            <h2 class="text-base font-bold text-slate-900">Event Order &amp; Tax Settings</h2>
            <p class="text-xs text-slate-500">
              Controls order numbering prefix (<code>{{ orderCodeInput || 'EVT' }}-{{ stationCodeInput || 'A' }}0001</code>) and local sales tax rate for new kiosk orders.
            </p>
          </div>
          <div class="flex flex-wrap items-end gap-3">
            <div>
              <label class="block text-xs text-slate-500 mb-1">Order Code</label>
              <input v-model="orderCodeInput" type="text" maxlength="8" class="w-28 border border-slate-300 rounded-lg px-3 py-1.5 text-sm font-mono uppercase" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">Station Code</label>
              <input v-model="stationCodeInput" type="text" maxlength="4" class="w-24 border border-slate-300 rounded-lg px-3 py-1.5 text-sm font-mono uppercase" />
            </div>
            <div>
              <label class="block text-xs text-slate-500 mb-1">Sales Tax (%)</label>
              <input v-model="taxRatePercentInput" type="number" step="0.01" min="0" max="25" class="w-28 border border-slate-300 rounded-lg px-3 py-1.5 text-sm font-mono" />
            </div>
            <button
              @click="saveEventOrderSettings"
              :disabled="isSavingSettings"
              class="px-4 py-1.5 text-sm font-semibold rounded-lg bg-slate-900 text-white hover:bg-slate-800 disabled:bg-slate-400"
            >
              {{ isSavingSettings ? "Saving..." : "Save Settings" }}
            </button>
          </div>
        </div>

        <!-- Product Catalog Table -->
        <div class="bg-white rounded-xl border border-slate-200 overflow-hidden">
          <div class="px-5 py-4 border-b border-slate-200 flex items-center justify-between">
            <div>
              <h2 class="text-base font-bold text-slate-900">Kiosk Product Catalog (v3 Pricing Ladder)</h2>
              <p class="text-xs text-slate-500">
                Changes apply to all LAN kiosks immediately (single digital, full digital bundle, +USB add-on, and lab prints).
              </p>
            </div>
            <button
              @click="showNewProductForm = !showNewProductForm"
              class="px-3.5 py-1.5 text-xs font-semibold rounded-lg bg-blue-600 text-white hover:bg-blue-700"
            >
              + Add Product
            </button>
          </div>

          <div v-if="showNewProductForm" class="p-4 bg-blue-50/60 border-b border-blue-200">
            <form @submit.prevent="addNewProduct" class="grid grid-cols-2 md:grid-cols-5 gap-3 items-end">
              <div>
                <label class="block text-xs text-slate-600 mb-1">Type</label>
                <select v-model="newProdType" class="w-full border border-slate-300 bg-white rounded px-2.5 py-1.5 text-sm">
                  <option value="print">Print</option>
                  <option value="single_digital">Single Digital</option>
                  <option value="full_digital">Full Digital</option>
                  <option value="usb_addon">USB Add-On</option>
                  <option value="custom">Custom</option>
                </select>
              </div>
              <div class="md:col-span-2">
                <label class="block text-xs text-slate-600 mb-1">Name *</label>
                <input v-model="newProdName" type="text" required placeholder="e.g. 8x10 Collage Print" class="w-full border border-slate-300 bg-white rounded px-2.5 py-1.5 text-sm" />
              </div>
              <div>
                <label class="block text-xs text-slate-600 mb-1">Size / Price ($)</label>
                <div class="flex gap-1.5">
                  <input v-model="newProdSize" type="text" placeholder="8x10" class="w-20 border border-slate-300 bg-white rounded px-2 py-1.5 text-sm" />
                  <input v-model="newProdPrice" type="number" step="0.01" min="0" required class="w-24 border border-slate-300 bg-white rounded px-2 py-1.5 text-sm font-mono" />
                </div>
              </div>
              <button type="submit" class="px-4 py-1.5 bg-blue-600 text-white text-sm font-semibold rounded-lg hover:bg-blue-700">
                Save Product
              </button>
            </form>
          </div>

          <div class="overflow-x-auto">
            <table class="min-w-full divide-y divide-slate-200 text-sm">
              <thead class="bg-slate-50 text-xs uppercase text-slate-500">
                <tr>
                  <th class="px-4 py-3 text-left font-medium">Type</th>
                  <th class="px-4 py-3 text-left font-medium">Display Name</th>
                  <th class="px-4 py-3 text-left font-medium">Size</th>
                  <th class="px-4 py-3 text-left font-medium">Price ($)</th>
                  <th class="px-4 py-3 text-center font-medium">Available</th>
                  <th class="px-4 py-3 text-right font-medium">Action</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-slate-100">
                <tr v-for="p in products" :key="p.id" class="hover:bg-slate-50/70">
                  <td class="px-4 py-2.5 font-mono text-xs text-slate-500">{{ p.productType }}</td>
                  <td class="px-4 py-2.5">
                    <input
                      v-model="editingNames[p.id]"
                      type="text"
                      class="w-full max-w-xs border border-slate-200 rounded px-2 py-1 text-sm"
                    />
                  </td>
                  <td class="px-4 py-2.5 text-slate-500 font-mono text-xs">{{ p.size || "—" }}</td>
                  <td class="px-4 py-2.5">
                    <input
                      v-model="editingPrices[p.id]"
                      type="number"
                      step="0.01"
                      min="0"
                      class="w-24 border border-slate-200 rounded px-2 py-1 text-sm font-mono"
                    />
                  </td>
                  <td class="px-4 py-2.5 text-center">
                    <input
                      type="checkbox"
                      :checked="p.isAvailable"
                      @change="saveProductRow(p, ($event.target as HTMLInputElement).checked)"
                      class="rounded cursor-pointer"
                    />
                  </td>
                  <td class="px-4 py-2.5 text-right">
                    <button
                      @click="saveProductRow(p)"
                      class="px-3 py-1 text-xs font-semibold rounded-md bg-slate-900 text-white hover:bg-slate-800"
                    >
                      Save
                    </button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
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
        <button
          @click="showQrModal = false"
          class="w-full py-2.5 rounded-xl bg-slate-900 text-white text-sm font-semibold hover:bg-slate-800"
        >
          Close
        </button>
      </div>
    </div>
  </div>
</template>
