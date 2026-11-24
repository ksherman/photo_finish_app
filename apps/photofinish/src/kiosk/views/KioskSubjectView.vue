<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { formatCents, useKioskStore } from "../store";
import type { KioskPhoto, KioskProduct, KioskSubject, KioskSubjectDetail } from "../types";

const props = defineProps<{
  subjectId: string;
}>();

const store = useKioskStore();

const loading = ref(true);
const error = ref<string | null>(null);
const subject = ref<KioskSubject | null>(null);
const photos = ref<KioskPhoto[]>([]);
const sections = ref<string[]>([]);
const activeSection = ref<string>("__all__");

// Lightbox state
const lightboxIndex = ref<number | null>(null);
const showPrintPicker = ref(false);
const toastText = ref<string | null>(null);
let toastTimer: ReturnType<typeof setTimeout> | null = null;
let touchStartX = 0;

function showToast(msg: string) {
  toastText.value = msg;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toastText.value = null;
  }, 2200);
}

async function fetchDetail() {
  loading.value = true;
  error.value = null;
  activeSection.value = "__all__";
  lightboxIndex.value = null;
  try {
    const res = await fetch(`/api/subjects/${encodeURIComponent(props.subjectId)}`);
    if (!res.ok) {
      throw new Error("Could not load photos for this competitor.");
    }
    const data: KioskSubjectDetail = await res.json();
    subject.value = data.subject;
    photos.value = data.photos;
    sections.value = data.sections;
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
}

const sectionCounts = computed(() => {
  const map: Record<string, number> = {};
  for (const p of photos.value) {
    map[p.sectionLabel] = (map[p.sectionLabel] || 0) + 1;
  }
  return map;
});

/// Ordered list of photos currently visible (used for lightbox index navigation!)
const visiblePhotos = computed(() => {
  if (activeSection.value === "__all__") return photos.value;
  return photos.value.filter((p) => p.sectionLabel === activeSection.value);
});

/// Grouped display when viewing "All" with multiple apparatuses/sections
const groupedSections = computed(() => {
  if (activeSection.value !== "__all__") {
    return [
      {
        label: activeSection.value,
        items: visiblePhotos.value.map((photo, flatIdx) => ({ photo, flatIdx })),
      },
    ];
  }
  const map = new Map<string, Array<{ photo: KioskPhoto; flatIdx: number }>>();
  visiblePhotos.value.forEach((photo, flatIdx) => {
    const list = map.get(photo.sectionLabel) || [];
    list.push({ photo, flatIdx });
    map.set(photo.sectionLabel, list);
  });
  return Array.from(map.entries()).map(([label, items]) => ({ label, items }));
});

const lightboxPhoto = computed(() =>
  lightboxIndex.value !== null ? visiblePhotos.value[lightboxIndex.value] || null : null
);

const hasFullPackage = computed(() =>
  subject.value ? store.hasFullDigitalForSubject(subject.value.id) : false
);

function openLightbox(idx: number) {
  lightboxIndex.value = idx;
  showPrintPicker.value = false;
  preloadNeighbors(idx);
}

function closeLightbox() {
  lightboxIndex.value = null;
  showPrintPicker.value = false;
}

function prevPhoto() {
  if (lightboxIndex.value !== null && lightboxIndex.value > 0) {
    lightboxIndex.value -= 1;
    showPrintPicker.value = false;
    preloadNeighbors(lightboxIndex.value);
  }
}

function nextPhoto() {
  if (lightboxIndex.value !== null && lightboxIndex.value < visiblePhotos.value.length - 1) {
    lightboxIndex.value += 1;
    showPrintPicker.value = false;
    preloadNeighbors(lightboxIndex.value);
  }
}

function preloadNeighbors(idx: number) {
  for (const offset of [-1, 1, 2]) {
    const p = visiblePhotos.value[idx + offset];
    if (p) {
      const img = new Image();
      img.src = `/photos/preview/${p.id}`;
    }
  }
}

function onTouchStart(e: TouchEvent) {
  touchStartX = e.changedTouches[0]?.clientX || 0;
}

function onTouchEnd(e: TouchEvent) {
  const endX = e.changedTouches[0]?.clientX || 0;
  const delta = endX - touchStartX;
  if (Math.abs(delta) > 50) {
    if (delta < 0) nextPhoto();
    else prevPhoto();
  }
}

function onKeyDown(e: KeyboardEvent) {
  if (lightboxIndex.value === null) return;
  if (e.key === "Escape") closeLightbox();
  else if (e.key === "ArrowLeft") prevPhoto();
  else if (e.key === "ArrowRight") nextPhoto();
}

function handleToggleFullPackage() {
  if (!subject.value) return;
  const wasInCart = hasFullPackage.value;
  store.toggleFullDigital(subject.value);
  showToast(
    wasInCart
      ? "Removed All Photos Package from cart"
      : `Added All ${photos.value.length} Photos for ${subject.value.displayName}`
  );
}

function handleToggleSingle(photo: KioskPhoto) {
  if (!subject.value) return;
  if (hasFullPackage.value) {
    showToast("Already included in your All Photos Package!");
    return;
  }
  const wasIn = store.hasSingleDigitalForPhoto(photo.id);
  store.toggleSingleDigital(subject.value, photo);
  if (!wasIn) {
    const upsell = store.upsellOpportunities.find((u) => u.subjectId === subject.value?.id);
    if (upsell) {
      showToast("Tip: 3+ photos selected — check Cart to upgrade to ALL photos!");
    } else {
      showToast("Added Single Digital to cart");
    }
  }
}

function handleAddPrint(photo: KioskPhoto, product: KioskProduct) {
  if (!subject.value) return;
  store.addPrint(subject.value, photo, product);
  showPrintPicker.value = false;
  showToast(`Added ${product.name} to cart`);
}

onMounted(() => {
  fetchDetail();
  window.addEventListener("keydown", onKeyDown);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeyDown);
});

watch(() => props.subjectId, fetchDetail);
</script>

<template>
  <div class="min-h-screen bg-slate-100 flex flex-col">
    <!-- Sticky Touch Header -->
    <header class="bg-white border-b border-slate-200 sticky top-0 z-20 shadow-xs">
      <div class="max-w-7xl mx-auto px-4 py-3 flex flex-wrap items-center justify-between gap-3">
        <div class="flex items-center gap-3 min-w-0">
          <button
            @click="store.navigate({ name: 'home' })"
            class="px-3.5 py-2.5 rounded-xl bg-slate-100 hover:bg-slate-200 active:scale-95 text-slate-700 font-semibold text-sm flex items-center gap-1.5 transition"
          >
            <span>&larr;</span>
            <span>Search</span>
          </button>

          <div v-if="subject" class="flex items-center gap-3 min-w-0">
            <div
              v-if="photos.length > 0"
              class="w-11 h-11 rounded-xl overflow-hidden bg-slate-100 border border-slate-200 flex-shrink-0"
            >
              <img
                :src="`/photos/avatar/${subject.id}`"
                decoding="async"
                class="w-full h-full object-cover object-top"
              />
            </div>
            <div class="min-w-0">
              <h1 class="text-lg sm:text-xl font-extrabold text-slate-900 truncate">
                {{ subject.displayName }}
              </h1>
              <p class="text-xs text-slate-500">
                {{ photos.length }} photos
                <span v-if="subject.groupLabel"> · {{ subject.groupLabel }}</span>
                <span v-if="subject.teamName"> · {{ subject.teamName }}</span>
              </p>
            </div>
          </div>
        </div>

        <div class="flex items-center gap-2.5">
          <button
            v-if="subject && store.fullDigitalProduct && photos.length > 0"
            @click="handleToggleFullPackage"
            class="px-4 py-2.5 rounded-xl font-semibold text-sm transition active:scale-95 shadow-xs"
            :class="
              hasFullPackage
                ? 'bg-emerald-600 text-white hover:bg-emerald-700'
                : 'bg-indigo-600 text-white hover:bg-indigo-700'
            "
          >
            <template v-if="hasFullPackage">
              &#10003; All Photos in Cart
            </template>
            <template v-else>
              All {{ photos.length }} Photos · {{ formatCents(store.fullDigitalProduct.priceCents) }}
            </template>
          </button>

          <button
            @click="store.navigate({ name: 'cart' })"
            class="px-4 py-2.5 rounded-xl border-2 font-semibold text-sm flex items-center gap-2 transition active:scale-95"
            :class="
              store.cartCount > 0
                ? 'border-indigo-600 bg-indigo-50 text-indigo-700 hover:bg-indigo-100'
                : 'border-slate-200 bg-white text-slate-600 hover:bg-slate-50'
            "
          >
            <span>Cart ({{ store.cartCount }})</span>
            <span v-if="store.cartCount > 0" class="font-mono text-xs bg-indigo-600 text-white px-2 py-0.5 rounded-full">
              {{ formatCents(store.subtotalCents) }}
            </span>
          </button>
        </div>
      </div>

      <!-- Apparatus / Section Filter Pills -->
      <div
        v-if="sections.length > 1"
        class="max-w-7xl mx-auto px-4 pb-3 flex items-center gap-2 overflow-x-auto"
      >
        <button
          @click="activeSection = '__all__'"
          class="px-4 py-2 rounded-full text-sm font-semibold whitespace-nowrap transition"
          :class="
            activeSection === '__all__'
              ? 'bg-slate-900 text-white'
              : 'bg-slate-100 text-slate-700 hover:bg-slate-200'
          "
        >
          All ({{ photos.length }})
        </button>
        <button
          v-for="sec in sections"
          :key="sec"
          @click="activeSection = sec"
          class="px-4 py-2 rounded-full text-sm font-semibold whitespace-nowrap transition"
          :class="
            activeSection === sec
              ? 'bg-slate-900 text-white'
              : 'bg-slate-100 text-slate-700 hover:bg-slate-200'
          "
        >
          {{ sec }} ({{ sectionCounts[sec] || 0 }})
        </button>
      </div>
    </header>

    <!-- 3-Photo Upsell Nudge right inside the gallery -->
    <div
      v-if="subject && store.upsellOpportunities.some((u) => u.subjectId === subject?.id)"
      class="bg-indigo-600 text-white px-4 py-3"
    >
      <div class="max-w-7xl mx-auto flex flex-wrap items-center justify-between gap-3 text-sm">
        <span class="font-medium">
          You've picked 3+ individual photos! Upgrade to <strong>ALL {{ photos.length }} photos</strong> for nearly the same price.
        </span>
        <button
          @click="store.upgradeSubjectToFullDigital(subject.id); showToast('Upgraded to All Photos Package!')"
          class="px-4 py-1.5 rounded-lg bg-white text-indigo-700 font-bold hover:bg-indigo-50"
        >
          Upgrade to All Photos ({{ store.fullDigitalProduct ? formatCents(store.fullDigitalProduct.priceCents) : "" }})
        </button>
      </div>
    </div>

    <!-- Main Gallery Body -->
    <main class="max-w-7xl w-full mx-auto px-4 py-6 flex-1 space-y-8">
      <div v-if="loading" class="py-20 text-center text-slate-500">Loading photos...</div>
      <div v-else-if="error" class="py-20 text-center text-red-600">{{ error }}</div>
      <div v-else-if="photos.length === 0" class="py-20 text-center text-slate-500">
        No photos available yet — check back shortly!
      </div>

      <section v-else v-for="group in groupedSections" :key="group.label" class="space-y-3">
        <div v-if="sections.length > 1 || activeSection !== '__all__'" class="flex items-baseline justify-between border-b border-slate-200 pb-2">
          <h2 class="text-lg font-bold text-slate-800">{{ group.label }}</h2>
          <span class="text-xs font-medium text-slate-500">{{ group.items.length }} photos</span>
        </div>

        <div class="grid grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 gap-2.5">
          <div
            v-for="{ photo, flatIdx } in group.items"
            :key="photo.id"
            class="relative group rounded-xl overflow-hidden bg-slate-200 aspect-[2/3] shadow-xs"
          >
            <button
              @click="openLightbox(flatIdx)"
              class="w-full h-full block focus:outline-none"
            >
              <img
                :src="`/photos/thumbnail/${photo.id}`"
                loading="lazy"
                class="w-full h-full object-cover group-hover:scale-[1.02] transition-transform duration-150"
              />
            </button>

            <!-- Quick Single Digital Select Badge -->
            <button
              v-if="store.singleDigitalProduct"
              @click.stop="handleToggleSingle(photo)"
              :title="hasFullPackage ? 'Included in All Photos Package' : 'Toggle Single Digital'"
              class="absolute bottom-2 right-2 w-10 h-10 rounded-full flex items-center justify-center font-bold text-lg shadow-md transition active:scale-90"
              :class="
                hasFullPackage || store.hasSingleDigitalForPhoto(photo.id)
                  ? 'bg-emerald-500 text-white'
                  : 'bg-white/90 text-slate-800 hover:bg-white'
              "
            >
              {{ hasFullPackage || store.hasSingleDigitalForPhoto(photo.id) ? "✓" : "+" }}
            </button>

            <!-- Print count badge if ordered as print -->
            <span
              v-if="store.printCountForPhoto(photo.id) > 0"
              class="absolute top-2 left-2 px-2 py-0.5 rounded-full bg-indigo-600 text-white text-xs font-bold shadow"
            >
              Print ×{{ store.printCountForPhoto(photo.id) }}
            </span>
          </div>
        </div>
      </section>
    </main>

    <!-- Full-Screen Touch Lightbox -->
    <div
      v-if="lightboxPhoto && lightboxIndex !== null"
      class="fixed inset-0 bg-black/95 z-50 flex flex-col"
      @touchstart.passive="onTouchStart"
      @touchend.passive="onTouchEnd"
    >
      <!-- Lightbox Top Bar -->
      <div class="px-4 py-3 flex items-center justify-between text-white/90">
        <div class="text-sm">
          <span class="font-semibold">{{ lightboxPhoto.sectionLabel }}</span>
          <span class="text-white/50 ml-2 font-mono text-xs">{{ lightboxPhoto.filename }}</span>
        </div>
        <div class="flex items-center gap-4">
          <span class="text-sm text-white/70 font-mono">
            {{ lightboxIndex + 1 }} / {{ visiblePhotos.length }}
          </span>
          <button
            @click="closeLightbox"
            class="w-11 h-11 rounded-full bg-white/15 hover:bg-white/25 text-white text-2xl flex items-center justify-center"
          >
            &times;
          </button>
        </div>
      </div>

      <!-- Lightbox Image Stage -->
      <div class="flex-1 relative flex items-center justify-center px-14 min-h-0">
        <button
          v-if="lightboxIndex > 0"
          @click="prevPhoto"
          class="absolute left-3 top-1/2 -translate-y-1/2 w-12 h-16 rounded-2xl bg-white/10 hover:bg-white/25 text-white text-3xl flex items-center justify-center"
        >
          &lsaquo;
        </button>

        <img
          :src="`/photos/preview/${lightboxPhoto.id}`"
          class="max-w-full max-h-full object-contain rounded-lg"
        />

        <button
          v-if="lightboxIndex < visiblePhotos.length - 1"
          @click="nextPhoto"
          class="absolute right-3 top-1/2 -translate-y-1/2 w-12 h-16 rounded-2xl bg-white/10 hover:bg-white/25 text-white text-3xl flex items-center justify-center"
        >
          &rsaquo;
        </button>
      </div>

      <!-- Lightbox Bottom Action Bar -->
      <div class="px-4 py-4 bg-slate-900/90 border-t border-white/10">
        <!-- Print Size Picker Popover -->
        <div
          v-if="showPrintPicker && store.printProducts.length > 0"
          class="max-w-2xl mx-auto mb-3 p-3 rounded-2xl bg-slate-800 flex flex-wrap items-center justify-center gap-2"
        >
          <span class="text-xs text-slate-300 mr-1">Choose Print Size:</span>
          <button
            v-for="p in store.printProducts"
            :key="p.id"
            @click="handleAddPrint(lightboxPhoto, p)"
            class="px-3.5 py-2 rounded-xl bg-white text-slate-900 font-semibold text-sm hover:bg-indigo-50 active:scale-95"
          >
            {{ p.size || p.name }} · {{ formatCents(p.priceCents) }}
          </button>
        </div>

        <div class="max-w-3xl mx-auto flex flex-wrap items-center justify-center gap-3">
          <button
            v-if="store.singleDigitalProduct"
            @click="handleToggleSingle(lightboxPhoto)"
            class="px-5 py-3 rounded-xl font-semibold text-sm transition active:scale-95"
            :class="
              hasFullPackage || store.hasSingleDigitalForPhoto(lightboxPhoto.id)
                ? 'bg-emerald-600 text-white'
                : 'bg-white text-slate-900 hover:bg-slate-100'
            "
          >
            <template v-if="hasFullPackage">✓ Included in All Photos</template>
            <template v-else-if="store.hasSingleDigitalForPhoto(lightboxPhoto.id)">
              ✓ Single Digital in Cart
            </template>
            <template v-else>
              + Single Digital ({{ formatCents(store.singleDigitalProduct.priceCents) }})
            </template>
          </button>

          <button
            v-if="store.printProducts.length > 0"
            @click="showPrintPicker = !showPrintPicker"
            class="px-5 py-3 rounded-xl bg-slate-700 text-white font-semibold text-sm hover:bg-slate-600 active:scale-95"
          >
            + Order Print {{ store.printCountForPhoto(lightboxPhoto.id) ? `(${store.printCountForPhoto(lightboxPhoto.id)})` : "" }}
          </button>

          <button
            v-if="store.fullDigitalProduct && !hasFullPackage"
            @click="handleToggleFullPackage"
            class="px-5 py-3 rounded-xl bg-indigo-600 text-white font-semibold text-sm hover:bg-indigo-500 active:scale-95"
          >
            All {{ photos.length }} Photos ({{ formatCents(store.fullDigitalProduct.priceCents) }})
          </button>
        </div>
      </div>
    </div>

    <!-- Subtle Floating Toast -->
    <div
      v-if="toastText"
      class="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 px-5 py-3 rounded-full bg-slate-900 text-white text-sm font-medium shadow-2xl"
    >
      {{ toastText }}
    </div>
  </div>
</template>
