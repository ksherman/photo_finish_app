<script setup lang="ts">
import { ref } from "vue";
import { formatCents, useKioskStore } from "../store";
import KioskTotalsRows from "../components/KioskTotalsRows.vue";

const store = useKioskStore();

const customerName = ref("");
const customerEmail = ref("");
const customerPhone = ref("");
const street = ref("");
const street2 = ref("");
const city = ref("");
const stateCode = ref("");
const zip = ref("");
const submitting = ref(false);
const errorMessage = ref<string | null>(null);

async function submit() {
  if (submitting.value) return;
  errorMessage.value = null;

  if (!customerName.value.trim()) {
    errorMessage.value = "Please enter your name.";
    return;
  }
  if (store.hasDigital && !customerEmail.value.trim()) {
    errorMessage.value = "Please enter your email so we can send your digital download link.";
    return;
  }
  if (
    store.hasPrints &&
    (!street.value.trim() || !city.value.trim() || !stateCode.value.trim() || !zip.value.trim())
  ) {
    errorMessage.value = "Please complete your shipping address for print delivery.";
    return;
  }

  submitting.value = true;
  try {
    await store.placeOrder({
      customerName: customerName.value.trim(),
      customerEmail: customerEmail.value.trim(),
      customerPhone: customerPhone.value.trim(),
      shippingAddress: store.hasPrints
        ? {
            street: street.value.trim(),
            street2: street2.value.trim() || undefined,
            city: city.value.trim(),
            state: stateCode.value.trim().toUpperCase(),
            zip: zip.value.trim(),
          }
        : null,
    });
  } catch (err) {
    errorMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <div class="min-h-screen bg-slate-100 flex flex-col">
    <header class="bg-white border-b border-slate-200 sticky top-0 z-10">
      <div class="max-w-2xl mx-auto px-4 py-4 flex items-center gap-4">
        <button
          @click="store.navigate({ name: 'cart' })"
          class="px-3.5 py-2 rounded-xl bg-slate-100 hover:bg-slate-200 text-slate-700 font-semibold text-sm"
        >
          &larr; Back to Cart
        </button>
        <h1 class="text-xl font-extrabold text-slate-900">Checkout</h1>
      </div>
    </header>

    <main class="max-w-2xl w-full mx-auto px-4 py-6 space-y-5">
      <div v-if="errorMessage" class="p-4 rounded-xl bg-red-50 border border-red-200 text-sm text-red-700 font-medium">
        {{ errorMessage }}
      </div>

      <!-- Summary Box -->
      <div class="bg-white rounded-2xl border border-slate-200 p-5 space-y-2">
        <KioskTotalsRows
          :subtotal-label="`Subtotal (${store.cartCount} item${store.cartCount === 1 ? '' : 's'})`"
          total-label="Total Due"
          :subtotal-cents="store.subtotalCents"
          :shipping-cents="store.shippingCents"
          :shipping-tier-label="store.shippingTierLabel"
          :tax-rate-basis-points="store.taxRateBasisPoints"
          :tax-cents="store.taxCents"
          :total-cents="store.totalCents"
        />
      </div>

      <!-- Contact & Shipping Form -->
      <form @submit.prevent="submit" class="bg-white rounded-2xl border border-slate-200 p-6 space-y-4">
        <div>
          <label class="block text-sm font-semibold text-slate-700 mb-1">
            Your Name <span class="text-red-500">*</span>
          </label>
          <input
            v-model="customerName"
            type="text"
            required
            placeholder="Jane Smith"
            class="w-full px-4 py-3.5 text-lg rounded-xl border-2 border-slate-200 focus:border-indigo-500 focus:outline-none"
          />
        </div>

        <div>
          <label class="block text-sm font-semibold text-slate-700 mb-1">
            Email Address <span v-if="store.hasDigital" class="text-red-500">*</span>
          </label>
          <input
            v-model="customerEmail"
            type="email"
            :required="store.hasDigital"
            placeholder="you@example.com"
            class="w-full px-4 py-3.5 text-lg rounded-xl border-2 border-slate-200 focus:border-indigo-500 focus:outline-none"
          />
          <p v-if="store.hasDigital" class="text-xs text-slate-500 mt-1">
            We'll email your Monday digital download link and gallery access here.
          </p>
        </div>

        <div>
          <label class="block text-sm font-semibold text-slate-700 mb-1">
            Phone <span class="text-slate-400 font-normal">(optional)</span>
          </label>
          <input
            v-model="customerPhone"
            type="tel"
            placeholder="(555) 123-4567"
            class="w-full px-4 py-3.5 text-lg rounded-xl border-2 border-slate-200 focus:border-indigo-500 focus:outline-none"
          />
        </div>

        <!-- Conditional Mailing Address for Direct Print Lab Drop-Ship -->
        <div v-if="store.hasPrints" class="pt-4 border-t border-slate-200 space-y-3">
          <div>
            <h2 class="text-base font-bold text-slate-900">
              Mailing Address <span class="text-red-500">*</span>
            </h2>
            <p class="text-xs text-slate-500">
              Required for print orders — prints ship directly to your home ($3.00 S&amp;H up to 8×10, $10.00 for 11×14+).
            </p>
          </div>
          <input
            v-model="street"
            type="text"
            required
            placeholder="Street Address *"
            class="w-full px-4 py-3 text-base rounded-xl border-2 border-slate-200 focus:border-indigo-500 focus:outline-none"
          />
          <input
            v-model="street2"
            type="text"
            placeholder="Apt / Suite (optional)"
            class="w-full px-4 py-3 text-base rounded-xl border-2 border-slate-200 focus:border-indigo-500 focus:outline-none"
          />
          <div class="grid grid-cols-6 gap-2.5">
            <input
              v-model="city"
              type="text"
              required
              placeholder="City *"
              class="col-span-3 px-4 py-3 text-base rounded-xl border-2 border-slate-200 focus:border-indigo-500 focus:outline-none"
            />
            <input
              v-model="stateCode"
              type="text"
              required
              maxlength="2"
              placeholder="State *"
              class="col-span-1 px-3 py-3 text-base rounded-xl border-2 border-slate-200 focus:border-indigo-500 focus:outline-none uppercase"
            />
            <input
              v-model="zip"
              type="text"
              required
              placeholder="ZIP *"
              class="col-span-2 px-4 py-3 text-base rounded-xl border-2 border-slate-200 focus:border-indigo-500 focus:outline-none"
            />
          </div>
        </div>

        <button
          type="submit"
          :disabled="submitting || store.cart.length === 0"
          class="w-full py-4 mt-2 rounded-2xl bg-indigo-600 text-white text-lg font-bold hover:bg-indigo-700 disabled:bg-slate-400 active:scale-[0.99] transition shadow-md"
        >
          {{ submitting ? "Placing Order..." : `Place Order · ${formatCents(store.totalCents)}` }}
        </button>
      </form>
    </main>
  </div>
</template>
