import type { ShippingAddress } from "@/types";

export function formatCents(cents: number): string {
  const sign = cents < 0 ? "-" : "";
  return `${sign}$${(Math.abs(cents) / 100).toFixed(2)}`;
}

export function formatBytes(bytes: number): string {
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

export function formatTime(iso: string): string {
  try {
    return new Date(iso).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

export function formatShippingAddress(addr: ShippingAddress): string {
  const s2 = addr.street2?.trim() ? ` ${addr.street2.trim()}` : "";
  return `${addr.street.trim()}${s2}, ${addr.city.trim()}, ${addr.state.trim()} ${addr.zip.trim()}`;
}

export function trimSlashes(path: string): string {
  return path.trim().replace(/^\/+|\/+$/g, "");
}

const PAYMENT_METHOD_LABELS: Record<string, string> = {
  square_qr: "Square QR",
  square_terminal: "Square Terminal",
  cash: "Cash",
  check: "Check",
  other: "Other",
};

export function paymentMethodLabel(method: string | null): string {
  return (method && PAYMENT_METHOD_LABELS[method]) || method || "—";
}

export function paymentStatusClass(status: string): string {
  if (status === "paid") return "bg-emerald-100 text-emerald-800";
  if (status === "refunded") return "bg-rose-100 text-rose-700";
  return "bg-amber-100 text-amber-800";
}
