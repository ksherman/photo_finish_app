<script setup lang="ts">
import { formatCents, useKioskStore } from "../store";

const store = useKioskStore();
</script>

<template>
  <div class="min-h-screen bg-slate-100 flex flex-col">
    <header class="bg-white border-b border-slate-200 sticky top-0 z-10">
      <div class="max-w-3xl mx-auto px-4 py-4 flex items-center justify-between gap-4">
        <button
          @click="store.navigate({ name: 'home' })"
          class="px-3.5 py-2 rounded-xl bg-slate-100 hover:bg-slate-200 text-slate-700 font-semibold text-sm"
        >
          &larr; Add Another {{ store.activeEvent?.subjectLabel || "Athlete" }}
        </button>
        <h1 class="text-xl font-extrabold text-slate-900">Your Cart ({{ store.cartCount }})</h1>
        <button
          v-if="store.cartCount > 0"
          @click="store.clearCart()"
          class="text-sm text-slate-400 hover:text-red-600"
        >
          Clear
        </button>
        <div v-else class="w-12" />
      </div>
    </header>

    <main class="max-w-3xl w-full mx-auto px-4 py-6 flex-1 space-y-5">
      <div v-if="store.cart.length === 0" class="bg-white rounded-2xl border border-slate-200 p-12 text-center space-y-4">
        <p class="text-lg font-semibold text-slate-700">Your cart is empty</p>
        <p class="text-sm text-slate-500">Search for your {{ (store.activeEvent?.subjectLabel || "athlete").toLowerCase() }} to browse and select photos.</p>
        <button
          @click="store.navigate({ name: 'home' })"
          class="px-6 py-3 rounded-xl bg-indigo-600 text-white font-semibold hover:bg-indigo-700"
        >
          Find Photos
        </button>
      </div>

      <template v-else>
        <!-- Smart 3-Photo Tipping-Point Upsell Banners -->
        <div
          v-for="up in store.upsellOpportunities"
          :key="up.subjectId"
          class="bg-indigo-600 text-white rounded-2xl p-5 shadow-md flex flex-wrap items-center justify-between gap-4"
        >
          <div class="space-y-1">
            <div class="text-xs uppercase tracking-wider font-bold text-indigo-200">
              Best Value Upgrade
            </div>
            <p class="font-bold text-base">
              You have {{ up.singleCount }} individual photos for {{ up.subjectDisplayName }} ({{ formatCents(up.singleTotalCents) }}).
            </p>
            <p class="text-sm text-indigo-100">
              Upgrade to <strong>ALL {{ up.subjectPhotoCount || "" }} photos</strong> for
              {{ up.diffCents > 0 ? `just ${formatCents(up.diffCents)} more` : "the exact same price or less" }}!
            </p>
          </div>
          <button
            @click="store.upgradeSubjectToFullDigital(up.subjectId)"
            class="px-5 py-3 rounded-xl bg-white text-indigo-700 font-extrabold text-sm hover:bg-indigo-50 active:scale-95 transition"
          >
            Upgrade to Full Package ({{ formatCents(up.fullPriceCents) }})
          </button>
        </div>

        <!-- Cart Items -->
        <div class="bg-white rounded-2xl border border-slate-200 divide-y divide-slate-100 overflow-hidden shadow-xs">
          <div
            v-for="item in store.cart"
            :key="item.key"
            class="p-4 flex items-center justify-between gap-4"
          >
            <div class="flex items-center gap-3.5 min-w-0">
              <img
                v-if="item.photoId"
                :src="`/photos/thumbnail/${item.photoId}`"
                class="w-14 h-16 object-cover rounded-lg bg-slate-200 flex-shrink-0"
              />
              <div
                v-else
                class="w-14 h-16 rounded-lg flex items-center justify-center flex-shrink-0 font-bold text-xs text-center px-1"
                :class="
                  item.productType === 'usb_addon'
                    ? 'bg-emerald-100 text-emerald-800'
                    : 'bg-indigo-100 text-indigo-800'
                "
              >
                {{ item.productType === "usb_addon" ? "USB-C/A" : "ALL" }}
              </div>

              <div class="min-w-0">
                <div class="font-bold text-slate-900 truncate">{{ item.productName }}</div>
                <div class="text-sm text-slate-500 truncate">
                  {{ item.subjectDisplayName }}
                  <span v-if="item.photoFilename" class="font-mono text-xs text-slate-400">
                    · {{ item.photoFilename }}
                  </span>
                  <span v-else-if="item.productType === 'full_digital' && item.subjectPhotoCount">
                    · All {{ item.subjectPhotoCount }} photos
                  </span>
                </div>
              </div>
            </div>

            <div class="flex items-center gap-3 flex-shrink-0">
              <!-- Quantity controls for prints -->
              <div v-if="item.productType === 'print'" class="flex items-center border border-slate-200 rounded-lg">
                <button
                  @click="store.updateItemQuantity(item.key, -1)"
                  class="px-2.5 py-1 text-slate-600 hover:bg-slate-100 font-bold"
                >
                  −
                </button>
                <span class="px-2.5 text-sm font-mono font-semibold">{{ item.quantity }}</span>
                <button
                  @click="store.updateItemQuantity(item.key, 1)"
                  class="px-2.5 py-1 text-slate-600 hover:bg-slate-100 font-bold"
                >
                  +
                </button>
              </div>

              <div class="text-right min-w-[4.5rem]">
                <div class="font-bold text-slate-900 font-mono">
                  {{ formatCents(item.unitPriceCents * item.quantity) }}
                </div>
              </div>

              <button
                @click="store.removeItem(item.key)"
                class="w-9 h-9 rounded-lg text-slate-400 hover:text-red-600 hover:bg-red-50 flex items-center justify-center text-xl"
                title="Remove"
              >
                &times;
              </button>
            </div>
          </div>
        </div>

        <!-- +$10 Instant Dual USB-C/A Add-On Card -->
        <div
          v-if="store.hasDigital && store.usbAddonProduct"
          @click="store.toggleUsbAddon()"
          class="rounded-2xl border-2 p-5 cursor-pointer transition flex items-center justify-between gap-4"
          :class="
            store.hasUsbAddon
              ? 'border-emerald-500 bg-emerald-50'
              : 'border-slate-200 bg-white hover:border-emerald-300'
          "
        >
          <div class="space-y-0.5">
            <div class="font-bold text-slate-900 flex items-center gap-2">
              <span>{{ store.usbAddonProduct.name }}</span>
              <span class="px-2 py-0.5 rounded-full text-xs font-bold bg-emerald-100 text-emerald-800">
                +{{ formatCents(store.usbAddonProduct.priceCents) }}
              </span>
            </div>
            <p class="text-sm text-slate-600">
              Take your photos home today on a fast dual USB-C/USB-A drive (plus Monday's online download link).
            </p>
          </div>
          <div
            class="w-8 h-8 rounded-full border-2 flex items-center justify-center font-bold flex-shrink-0"
            :class="
              store.hasUsbAddon
                ? 'border-emerald-600 bg-emerald-600 text-white'
                : 'border-slate-300 text-transparent'
            "
          >
            ✓
          </div>
        </div>

        <!-- Totals & Proceed -->
        <div class="bg-white rounded-2xl border border-slate-200 p-5 space-y-3 shadow-xs">
          <div class="flex justify-between text-sm text-slate-600">
            <span>Subtotal</span>
            <span class="font-mono">{{ formatCents(store.subtotalCents) }}</span>
          </div>
          <div class="flex justify-between text-sm text-slate-600">
            <span>Tax ({{ (store.taxRateBasisPoints / 100).toFixed(2) }}%)</span>
            <span class="font-mono">{{ formatCents(store.taxCents) }}</span>
          </div>
          <div class="border-t border-slate-200 pt-3 flex justify-between text-lg font-extrabold text-slate-900">
            <span>Total</span>
            <span class="font-mono">{{ formatCents(store.totalCents) }}</span>
          </div>

          <button
            @click="store.navigate({ name: 'checkout' })"
            class="w-full mt-2 py-4 rounded-2xl bg-indigo-600 text-white text-lg font-bold hover:bg-indigo-700 active:scale-[0.99] transition shadow-md"
          >
            Proceed to Checkout &rarr;
          </button>
        </div>
      </template>
    </main>
  </div>
</template>
