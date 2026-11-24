<script setup lang="ts">
import { formatCents } from "@/utils/format";

defineProps<{
  subtotalLabel?: string;
  totalLabel?: string;
  subtotalCents: number;
  shippingCents: number;
  shippingTierLabel?: string;
  taxRateBasisPoints: number;
  taxCents: number;
  totalCents: number;
}>();
</script>

<template>
  <div class="flex justify-between text-sm text-slate-600">
    <span>{{ subtotalLabel || "Subtotal" }}</span>
    <span class="font-mono">{{ formatCents(subtotalCents) }}</span>
  </div>
  <div v-if="shippingCents > 0" class="flex justify-between text-sm text-slate-600">
    <span>
      Shipping &amp; Handling
      <span class="text-xs text-slate-400">
        ({{ shippingTierLabel ? `${shippingTierLabel} · ` : "" }}not taxed)
      </span>
    </span>
    <span class="font-mono">{{ formatCents(shippingCents) }}</span>
  </div>
  <div class="flex justify-between text-sm text-slate-600">
    <span>Tax ({{ (taxRateBasisPoints / 100).toFixed(2) }}%)</span>
    <span class="font-mono">{{ formatCents(taxCents) }}</span>
  </div>
  <div class="border-t border-slate-200 pt-2.5 flex justify-between text-lg font-extrabold text-slate-900">
    <span>{{ totalLabel || "Total" }}</span>
    <span class="font-mono">{{ formatCents(totalCents) }}</span>
  </div>
</template>
