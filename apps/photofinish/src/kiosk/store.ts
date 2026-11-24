import { defineStore } from "pinia";
import { formatCents } from "@/utils/format";
import type {
  CartItem,
  KioskEventSummary,
  KioskOrder,
  KioskPhoto,
  KioskProduct,
  KioskRoute,
  KioskStatusResponse,
  KioskSubject,
  ShippingAddress,
} from "./types";

export { formatCents };

const CART_STORAGE_KEY = "photofinish_kiosk_cart_v1";

/** Flat untaxed S&H for print orders up to 8×10 ($3.00). */
export const SMALL_PRINT_SHIPPING_CENTS = 300;
/** Flat untaxed S&H for print orders with any print 11×14 or larger ($10.00). */
export const LARGE_PRINT_SHIPPING_CENTS = 1000;

export function parseMaxPrintDimensionInches(text: string | null | undefined): number | null {
  if (!text) return null;
  const re = /(\d+)\s*[xX×]\s*(\d+)/g;
  let best: number | null = null;
  let match: RegExpExecArray | null;
  while ((match = re.exec(text)) !== null) {
    const a = parseInt(match[1], 10);
    const b = parseInt(match[2], 10);
    if (!Number.isNaN(a) && !Number.isNaN(b)) {
      const maxEdge = Math.max(a, b);
      best = best === null ? maxEdge : Math.max(best, maxEdge);
    }
  }
  return best;
}

export function calculatePrintShippingCents(items: CartItem[]): number {
  let hasPrint = false;
  let hasLargePrint = false;
  for (const item of items) {
    if (item.productType !== "print") continue;
    hasPrint = true;
    const maxDim =
      parseMaxPrintDimensionInches(item.productSize) ??
      parseMaxPrintDimensionInches(item.productName);
    if (maxDim !== null && maxDim >= 11) {
      hasLargePrint = true;
    }
  }
  if (!hasPrint) return 0;
  return hasLargePrint ? LARGE_PRINT_SHIPPING_CENTS : SMALL_PRINT_SHIPPING_CENTS;
}

export function parseRoute(pathname: string): KioskRoute {
  const clean = pathname.replace(/\/+$/, "") || "/";
  if (clean === "/") return { name: "home" };
  if (clean === "/cart") return { name: "cart" };
  if (clean === "/checkout") return { name: "checkout" };
  const subMatch = clean.match(/^\/subject\/([^/]+)$/);
  if (subMatch) return { name: "subject", subjectId: decodeURIComponent(subMatch[1]) };
  const ordMatch = clean.match(/^\/order\/([^/]+)$/);
  if (ordMatch) return { name: "order", orderNumber: decodeURIComponent(ordMatch[1]) };
  return { name: "home" };
}

function routeToPath(route: KioskRoute): string {
  switch (route.name) {
    case "home":
      return "/";
    case "subject":
      return `/subject/${encodeURIComponent(route.subjectId)}`;
    case "cart":
      return "/cart";
    case "checkout":
      return "/checkout";
    case "order":
      return `/order/${encodeURIComponent(route.orderNumber)}`;
  }
}

function loadSavedCart(): CartItem[] {
  try {
    const raw = sessionStorage.getItem(CART_STORAGE_KEY);
    return raw ? (JSON.parse(raw) as CartItem[]) : [];
  } catch {
    return [];
  }
}

function makeCartItem(
  key: string,
  prod: KioskProduct,
  extra: Partial<
    Pick<
      CartItem,
      "subjectId" | "subjectDisplayName" | "subjectPhotoCount" | "photoId" | "photoFilename"
    >
  > = {}
): CartItem {
  return {
    key,
    productId: prod.id,
    productType: prod.productType,
    productName: prod.name,
    productSize: prod.size,
    unitPriceCents: prod.priceCents,
    quantity: 1,
    subjectId: extra.subjectId ?? null,
    subjectDisplayName: extra.subjectDisplayName ?? null,
    subjectPhotoCount: extra.subjectPhotoCount,
    photoId: extra.photoId ?? null,
    photoFilename: extra.photoFilename ?? null,
  };
}

export const useKioskStore = defineStore("kiosk", {
  state: () => ({
    route: parseRoute(window.location.pathname) as KioskRoute,
    activeEvent: null as KioskEventSummary | null,
    subjects: [] as KioskSubject[],
    showcasePhotoIds: [] as string[],
    products: [] as KioskProduct[],
    taxRateBasisPoints: 850,
    cart: loadSavedCart() as CartItem[],
    lastOrder: null as KioskOrder | null,
    loading: false,
    rosterLoaded: false,
  }),

  getters: {
    singleDigitalProduct: (s) =>
      s.products.find((p) => p.productType === "single_digital") || null,
    fullDigitalProduct: (s) =>
      s.products.find((p) => p.productType === "full_digital") || null,
    usbAddonProduct: (s) =>
      s.products.find((p) => p.productType === "usb_addon") || null,
    printProducts: (s) => s.products.filter((p) => p.productType === "print"),

    cartCount: (s) => s.cart.reduce((sum, item) => sum + item.quantity, 0),
    subtotalCents: (s) =>
      s.cart.reduce((sum, item) => sum + item.unitPriceCents * item.quantity, 0),
    shippingCents: (s) => calculatePrintShippingCents(s.cart),
    shippingTierLabel(): string {
      if (this.shippingCents === LARGE_PRINT_SHIPPING_CENTS) return "Prints 11×14+";
      if (this.shippingCents === SMALL_PRINT_SHIPPING_CENTS) return "Prints up to 8×10";
      return "";
    },
    taxCents(): number {
      // Sales tax applies strictly to product subtotal — shipping & handling is untaxed.
      return Math.round((this.subtotalCents * this.taxRateBasisPoints) / 10_000);
    },
    totalCents(): number {
      return this.subtotalCents + this.shippingCents + this.taxCents;
    },
    hasPrints: (s) => s.cart.some((i) => i.productType === "print"),
    hasDigital: (s) =>
      s.cart.some((i) => i.productType === "full_digital" || i.productType === "single_digital"),
    hasUsbAddon: (s) => s.cart.some((i) => i.productType === "usb_addon"),

    hasFullDigitalForSubject: (s) => (subjectId: string) =>
      s.cart.some((i) => i.productType === "full_digital" && i.subjectId === subjectId),
    hasSingleDigitalForPhoto: (s) => (photoId: string) =>
      s.cart.some((i) => i.productType === "single_digital" && i.photoId === photoId),
    printCountForPhoto: (s) => (photoId: string) =>
      s.cart
        .filter((i) => i.productType === "print" && i.photoId === photoId)
        .reduce((sum, i) => sum + i.quantity, 0),

    /// Detects any subject with >= 3 single digital photos who doesn't yet have the Full Digital Package.
    upsellOpportunities(): Array<{
      subjectId: string;
      subjectDisplayName: string;
      singleCount: number;
      singleTotalCents: number;
      fullPriceCents: number;
      diffCents: number;
      subjectPhotoCount?: number;
    }> {
      const fullProd = this.fullDigitalProduct;
      if (!fullProd) return [];

      const bySubject = new Map<
        string,
        { displayName: string; count: number; cents: number; photoCount?: number }
      >();
      for (const item of this.cart) {
        if (item.productType !== "single_digital" || !item.subjectId) continue;
        if (this.hasFullDigitalForSubject(item.subjectId)) continue;
        const cur = bySubject.get(item.subjectId) || {
          displayName: item.subjectDisplayName || "Athlete",
          count: 0,
          cents: 0,
          photoCount: item.subjectPhotoCount,
        };
        cur.count += item.quantity;
        cur.cents += item.unitPriceCents * item.quantity;
        bySubject.set(item.subjectId, cur);
      }

      const opportunities = [];
      for (const [subjectId, info] of bySubject.entries()) {
        if (info.count >= 3) {
          opportunities.push({
            subjectId,
            subjectDisplayName: info.displayName,
            singleCount: info.count,
            singleTotalCents: info.cents,
            fullPriceCents: fullProd.priceCents,
            diffCents: fullProd.priceCents - info.cents,
            subjectPhotoCount: info.photoCount,
          });
        }
      }
      return opportunities;
    },
  },

  actions: {
    persistCart() {
      try {
        sessionStorage.setItem(CART_STORAGE_KEY, JSON.stringify(this.cart));
      } catch {
        // Ignore storage quota issues in private browsing
      }
    },

    navigate(route: KioskRoute) {
      this.route = route;
      const target = routeToPath(route);
      if (window.location.pathname !== target) {
        window.history.pushState({}, "", target);
      }
      window.scrollTo({ top: 0, behavior: "instant" as ScrollBehavior });
    },

    syncRouteFromWindow() {
      this.route = parseRoute(window.location.pathname);
    },

    async bootstrap() {
      this.loading = true;
      try {
        const statusRes = await fetch("/api/status");
        if (statusRes.ok) {
          const data: KioskStatusResponse = await statusRes.json();
          this.activeEvent = data.activeEvent;
          if (data.activeEvent) {
            this.taxRateBasisPoints = data.activeEvent.taxRateBasisPoints;
          }
        }
        if (this.activeEvent) {
          await Promise.all([this.loadProducts(), this.loadRoster(), this.loadShowcase()]);
        }
      } finally {
        this.loading = false;
      }
    },

    async loadProducts() {
      const res = await fetch("/api/products");
      if (res.ok) {
        const data = await res.json();
        this.products = data.products || [];
        if (typeof data.taxRateBasisPoints === "number") {
          this.taxRateBasisPoints = data.taxRateBasisPoints;
        }
      }
    },

    async loadRoster() {
      const res = await fetch("/api/subjects?limit=10000");
      if (res.ok) {
        this.subjects = await res.json();
        this.rosterLoaded = true;
      }
    },

    async loadShowcase() {
      const res = await fetch("/api/kiosk/showcase");
      if (res.ok) {
        const data = await res.json();
        this.showcasePhotoIds = data.photoIds || [];
      }
    },

    setFullDigitalForSubject(subjectId: string, displayName: string, photoCount?: number) {
      const prod = this.fullDigitalProduct;
      if (!prod) return;
      // Remove any redundant single digitals for this subject automatically
      this.cart = this.cart.filter(
        (i) => !(i.productType === "single_digital" && i.subjectId === subjectId)
      );
      const key = `full:${subjectId}`;
      if (!this.cart.some((i) => i.key === key)) {
        this.cart.push(
          makeCartItem(key, prod, {
            subjectId,
            subjectDisplayName: displayName,
            subjectPhotoCount: photoCount,
          })
        );
      }
    },

    toggleFullDigital(subject: KioskSubject) {
      const key = `full:${subject.id}`;
      const existingIdx = this.cart.findIndex((i) => i.key === key);
      if (existingIdx >= 0) {
        this.cart.splice(existingIdx, 1);
      } else {
        this.setFullDigitalForSubject(subject.id, subject.displayName, subject.photoCount);
      }
      this.cleanupUsbIfNoDigital();
      this.persistCart();
    },

    upgradeSubjectToFullDigital(subjectId: string) {
      const sample = this.cart.find(
        (i) => i.productType === "single_digital" && i.subjectId === subjectId
      );
      this.setFullDigitalForSubject(
        subjectId,
        sample?.subjectDisplayName || "Athlete",
        sample?.subjectPhotoCount
      );
      this.persistCart();
    },

    toggleSingleDigital(subject: KioskSubject, photo: KioskPhoto) {
      const prod = this.singleDigitalProduct;
      if (!prod || this.hasFullDigitalForSubject(subject.id)) return;

      const key = `single:${photo.id}`;
      const existingIdx = this.cart.findIndex((i) => i.key === key);
      if (existingIdx >= 0) {
        this.cart.splice(existingIdx, 1);
      } else {
        this.cart.push(
          makeCartItem(key, prod, {
            subjectId: subject.id,
            subjectDisplayName: subject.displayName,
            subjectPhotoCount: subject.photoCount,
            photoId: photo.id,
            photoFilename: photo.filename,
          })
        );
      }
      this.cleanupUsbIfNoDigital();
      this.persistCart();
    },

    addPrint(subject: KioskSubject, photo: KioskPhoto, product: KioskProduct) {
      const key = `print:${photo.id}:${product.id}`;
      const existing = this.cart.find((i) => i.key === key);
      if (existing) {
        existing.quantity += 1;
      } else {
        this.cart.push(
          makeCartItem(key, product, {
            subjectId: subject.id,
            subjectDisplayName: subject.displayName,
            photoId: photo.id,
            photoFilename: photo.filename,
          })
        );
      }
      this.persistCart();
    },

    toggleUsbAddon() {
      const prod = this.usbAddonProduct;
      if (!prod) return;
      const key = "usb_addon";
      const idx = this.cart.findIndex((i) => i.key === key);
      if (idx >= 0) {
        this.cart.splice(idx, 1);
      } else {
        const firstSubject = this.cart.find((i) => i.subjectId);
        this.cart.push(
          makeCartItem(key, prod, {
            subjectId: firstSubject?.subjectId ?? null,
            subjectDisplayName: firstSubject?.subjectDisplayName ?? null,
          })
        );
      }
      this.persistCart();
    },

    updateItemQuantity(key: string, delta: number) {
      const item = this.cart.find((i) => i.key === key);
      if (!item) return;
      const next = item.quantity + delta;
      if (next <= 0) {
        this.removeItem(key);
      } else {
        item.quantity = next;
        this.persistCart();
      }
    },

    removeItem(key: string) {
      this.cart = this.cart.filter((i) => i.key !== key);
      this.cleanupUsbIfNoDigital();
      this.persistCart();
    },

    cleanupUsbIfNoDigital() {
      if (!this.hasDigital) {
        this.cart = this.cart.filter((i) => i.productType !== "usb_addon");
      }
    },

    clearCart() {
      this.cart = [];
      this.persistCart();
    },

    async placeOrder(payload: {
      customerName: string;
      customerEmail: string;
      customerPhone?: string;
      shippingAddress?: ShippingAddress | null;
      notes?: string;
    }): Promise<KioskOrder> {
      const res = await fetch("/api/orders", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          customerName: payload.customerName,
          customerEmail: payload.customerEmail || null,
          customerPhone: payload.customerPhone || null,
          shippingAddress: payload.shippingAddress || null,
          notes: payload.notes || null,
          items: this.cart.map((item) => ({
            productId: item.productId,
            subjectId: item.subjectId,
            photoId: item.photoId,
            quantity: item.quantity,
          })),
        }),
      });
      if (!res.ok) {
        let message = "Failed to place order";
        try {
          const errBody = await res.json();
          if (errBody.error) message = errBody.error;
        } catch {
          // ignore
        }
        throw new Error(message);
      }
      const order: KioskOrder = await res.json();
      this.lastOrder = order;
      this.clearCart();
      this.navigate({ name: "order", orderNumber: order.orderNumber });
      return order;
    },
  },
});
