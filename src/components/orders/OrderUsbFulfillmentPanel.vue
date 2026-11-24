<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { Channel, invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { formatBytes } from "@/utils/format";
import type {
  OrderRecord,
  UsbCopyProgressEvent,
  UsbCopyResult,
  UsbDriveInfo,
  UsbOrderPlan,
} from "@/types";

const props = defineProps<{
  order: OrderRecord;
  hasPendingUsb: boolean;
  usbDrives: UsbDriveInfo[];
}>();

const emit = defineEmits<{
  refreshDrives: [];
  copied: [message: string];
  banner: [message: string];
  error: [message: string];
  copyingChange: [copying: boolean];
}>();

const usbPlan = ref<UsbOrderPlan | null>(null);
const loadingUsbPlan = ref(false);
const selectedUsbMount = ref("");
const selectedUsbDiskId = ref<string | null>(null);
const autoEjectUsb = ref(true);
const autoMarkUsbFulfilled = ref(true);
const isCopyingUsb = ref(false);
const usbProgress = ref<UsbCopyProgressEvent | null>(null);

const selectedUsbDriveObj = computed(
  () => props.usbDrives.find((d) => d.mountPoint === selectedUsbMount.value) || null
);

const usbInsufficientSpace = computed(() => {
  if (!selectedUsbDriveObj.value || !usbPlan.value) return false;
  if (selectedUsbDriveObj.value.freeBytes <= 0) return false;
  return usbPlan.value.totalBytes > selectedUsbDriveObj.value.freeBytes;
});

async function loadUsbPlan() {
  loadingUsbPlan.value = true;
  try {
    usbPlan.value = await invoke<UsbOrderPlan>("get_order_usb_plan", {
      orderId: props.order.id,
    });
  } catch (err) {
    usbPlan.value = null;
    emit("error", String(err));
  } finally {
    loadingUsbPlan.value = false;
  }
}

watch(
  () => props.order.id,
  () => loadUsbPlan(),
  { immediate: true }
);

watch(
  () => props.usbDrives,
  (drives) => {
    if (!selectedUsbMount.value) {
      const preferred = drives.find((d) => !d.hasDcim) || drives[0];
      if (preferred) {
        selectedUsbMount.value = preferred.mountPoint;
        selectedUsbDiskId.value = preferred.diskId;
      }
    } else {
      const match = drives.find((d) => d.mountPoint === selectedUsbMount.value);
      if (match) selectedUsbDiskId.value = match.diskId;
    }
  },
  { immediate: true }
);

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
    const match = props.usbDrives.find((d) => d.mountPoint === picked);
    selectedUsbDiskId.value = match ? match.diskId : null;
  }
}

async function startUsbCopy() {
  if (!selectedUsbMount.value || isCopyingUsb.value) return;
  isCopyingUsb.value = true;
  emit("copyingChange", true);
  usbProgress.value = null;

  try {
    const channel = new Channel<UsbCopyProgressEvent>();
    channel.onmessage = (ev) => {
      usbProgress.value = ev;
    };

    const res = await invoke<UsbCopyResult>("copy_order_to_usb", {
      request: {
        orderId: props.order.id,
        destinationDir: selectedUsbMount.value,
        ejectDiskId: autoEjectUsb.value ? selectedUsbDiskId.value : null,
        autoMarkFulfilled: autoMarkUsbFulfilled.value,
      },
      onProgress: channel,
    });

    const ejectNote = res.ejected
      ? " — drive safely ejected!"
      : res.ejectWarning
        ? ` (copy succeeded, eject warning: ${res.ejectWarning})`
        : "";
    const msg = `Copied ${res.filesCopied} photo(s) (${formatBytes(res.bytesCopied)}) for ${props.order.orderNumber} in ${res.elapsedSecs.toFixed(1)}s (${res.mbPerSec.toFixed(1)} MB/s)${ejectNote}`;

    if (res.ejected) {
      selectedUsbMount.value = "";
      selectedUsbDiskId.value = null;
    }
    emit("copied", msg);
  } catch (err) {
    emit("error", String(err));
  } finally {
    isCopyingUsb.value = false;
    emit("copyingChange", false);
    usbProgress.value = null;
  }
}

async function handleManualEject(drive: UsbDriveInfo) {
  try {
    await invoke("eject_usb_drive", { diskId: drive.diskId });
    if (selectedUsbMount.value === drive.mountPoint) {
      selectedUsbMount.value = "";
      selectedUsbDiskId.value = null;
    }
    emit("banner", `Safely ejected "${drive.volumeName}".`);
    emit("refreshDrives");
  } catch (err) {
    emit("error", String(err));
  }
}
</script>

<template>
  <div class="p-5 space-y-3.5" :class="hasPendingUsb ? 'bg-indigo-50/40' : ''">
    <div class="flex items-center justify-between gap-2">
      <div>
        <h3 class="text-sm font-bold text-slate-900 flex items-center gap-2">
          <span>Instant Dual USB-C/A Fulfillment</span>
          <span
            v-if="hasPendingUsb"
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
        @click="emit('refreshDrives')"
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
      <div
        v-for="subj in usbPlan.subjects"
        :key="subj.subjectLabel"
        class="pl-2 border-l-2 border-indigo-200 space-y-0.5"
      >
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
        <button @click="pickCustomUsbFolder" class="text-xs font-medium text-blue-600 hover:underline">
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

    <!-- Live Progress -->
    <div v-if="isCopyingUsb && usbProgress" class="space-y-1.5">
      <div class="flex justify-between text-xs font-medium text-indigo-800">
        <span>Copying {{ usbProgress.copiedFiles }} / {{ usbProgress.totalFiles }} ({{ usbProgress.currentFile }})</span>
        <span class="font-mono">{{ usbProgress.mbPerSec.toFixed(1) }} MB/s · {{ Math.round(usbProgress.percentage) }}%</span>
      </div>
      <div class="w-full h-2.5 bg-indigo-100 rounded-full overflow-hidden">
        <div
          class="h-full bg-indigo-600 transition-all duration-150"
          :style="{ width: `${usbProgress.percentage}%` }"
        />
      </div>
    </div>

    <!-- Controls -->
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
</template>
