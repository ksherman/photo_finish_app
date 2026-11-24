<script setup lang="ts">
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useEventStore } from "@/stores/event";
import { formatCents } from "@/utils/format";
import type { EventSummary, ProductRecord } from "@/types";

const emit = defineEmits<{
  banner: [message: string];
  error: [message: string];
}>();

const eventStore = useEventStore();

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

async function loadCatalog() {
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
    emit("error", String(err));
  }
}

async function saveProductRow(p: ProductRecord, overrideAvailable?: boolean) {
  const dollars = parseFloat(editingPrices.value[p.id] ?? "");
  if (Number.isNaN(dollars) || dollars < 0) {
    emit("error", `Invalid price for "${p.name}"`);
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
    emit(
      "banner",
      `Updated "${editingNames.value[p.id] || p.name}" (${formatCents(priceCents)}). Kiosks will reflect this immediately.`
    );
    await loadCatalog();
  } catch (err) {
    emit("error", String(err));
  }
}

async function addNewProduct() {
  const dollars = parseFloat(newProdPrice.value);
  if (!newProdName.value.trim() || Number.isNaN(dollars) || dollars < 0) {
    emit("error", "Provide a valid product name and price.");
    return;
  }
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
    emit("banner", "Added new product to event catalog.");
    await loadCatalog();
  } catch (err) {
    emit("error", String(err));
  }
}

async function saveEventOrderSettings() {
  if (isSavingSettings.value) return;
  const pct = parseFloat(taxRatePercentInput.value);
  if (Number.isNaN(pct) || pct < 0 || pct > 25) {
    emit("error", "Tax rate must be between 0% and 25%.");
    return;
  }
  isSavingSettings.value = true;
  try {
    const updated = await invoke<EventSummary>("update_event_order_settings", {
      input: {
        orderCode: orderCodeInput.value.trim(),
        stationCode: stationCodeInput.value.trim(),
        taxRateBasisPoints: Math.round(pct * 100),
      },
    });
    eventStore.activeEvent = updated;
    emit(
      "banner",
      `Updated event order settings (${updated.orderCode}-${updated.stationCode} · ${(updated.taxRateBasisPoints / 100).toFixed(2)}% tax).`
    );
  } catch (err) {
    emit("error", String(err));
  } finally {
    isSavingSettings.value = false;
  }
}

watch(() => eventStore.activeEvent?.id, loadCatalog, { immediate: true });
defineExpose({ loadCatalog });
</script>

<template>
  <div class="space-y-5">
    <!-- Event Order Code & Tax Settings -->
    <div class="bg-white rounded-xl border border-slate-200 p-5 space-y-3">
      <div>
        <h2 class="text-base font-bold text-slate-900">Event Order &amp; Tax Settings</h2>
        <p class="text-xs text-slate-500">
          Controls order numbering prefix (<code>{{ orderCodeInput || "EVT" }}-{{ stationCodeInput || "A" }}0001</code>) and local sales tax rate for new kiosk orders. Print orders automatically require a mailing address and add flat untaxed S&amp;H (<strong>$3.00</strong> up to 8×10, <strong>$10.00</strong> for 11×14+).
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
                <input v-model="editingNames[p.id]" type="text" class="w-full max-w-xs border border-slate-200 rounded px-2 py-1 text-sm" />
              </td>
              <td class="px-4 py-2.5 text-slate-500 font-mono text-xs">{{ p.size || "—" }}</td>
              <td class="px-4 py-2.5">
                <input v-model="editingPrices[p.id]" type="number" step="0.01" min="0" class="w-24 border border-slate-200 rounded px-2 py-1 text-sm font-mono" />
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
                <button @click="saveProductRow(p)" class="px-3 py-1 text-xs font-semibold rounded-md bg-slate-900 text-white hover:bg-slate-800">
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
