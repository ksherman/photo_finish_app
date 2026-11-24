import type {
  PhotoCounts,
  ProductRecord,
  ShippingAddress,
  SubjectRecord,
} from "@/types";

export type { ShippingAddress };
export type KioskSubject = SubjectRecord;
export type KioskProduct = ProductRecord;

export interface KioskEventSummary {
  id: string;
  name: string;
  subjectLabel: string;
  orderCode: string;
  stationCode: string;
  taxRateBasisPoints: number;
  photoCounts: PhotoCounts;
  subjectCount: number;
}

export interface KioskStatusResponse {
  service: string;
  version: string;
  activeEvent: KioskEventSummary | null;
}

export interface KioskPhoto {
  id: string;
  subjectId: string | null;
  relativePath: string;
  folderPath: string;
  leafFolder: string | null;
  filename: string;
  status: string;
  width: number | null;
  height: number | null;
  sectionLabel: string;
  tags: Record<string, string>;
}

export interface KioskSubjectDetail {
  subject: KioskSubject;
  photos: KioskPhoto[];
  sections: string[];
}

export interface CartItem {
  key: string;
  productId: string;
  productType: string;
  productName: string;
  productSize: string | null;
  unitPriceCents: number;
  quantity: number;
  subjectId: string | null;
  subjectDisplayName: string | null;
  subjectPhotoCount?: number;
  photoId: string | null;
  photoFilename: string | null;
}

export interface KioskOrderItem {
  id: string;
  productType: string;
  productName: string;
  productSize: string | null;
  subjectDisplayName: string | null;
  photoFilename: string | null;
  quantity: number;
  unitPriceCents: number;
  lineTotalCents: number;
}

export interface KioskOrder {
  id: string;
  orderNumber: string;
  customerName: string;
  customerEmail: string | null;
  customerPhone: string | null;
  shippingAddress: ShippingAddress | null;
  subtotalCents: number;
  shippingCents: number;
  taxRateBasisPoints: number;
  taxCents: number;
  totalCents: number;
  paymentStatus: string;
  items: KioskOrderItem[];
  squarePosUrl: string;
  createdAt: string;
}

export type KioskRoute =
  | { name: "home" }
  | { name: "subject"; subjectId: string }
  | { name: "cart" }
  | { name: "checkout" }
  | { name: "order"; orderNumber: string };
