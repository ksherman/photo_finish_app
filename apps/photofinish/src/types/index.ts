export interface EventPreset {
  id: string;
  label: string;
  description: string;
  subject_label: string;
  folder_levels: string[];
}

export interface ScaffoldConfig {
  num_areas?: number;
  area_prefix?: string;
  sessions_per_area?: number;
  session_prefix?: string;
}

export interface CreateEventParams {
  rootPath: string;
  name: string;
  description?: string;
  presetId?: string;
  subjectLabel?: string;
  folderLevels?: string[];
  orderCode?: string;
  stationCode?: string;
  taxRateBasisPoints?: number;
  scaffold?: ScaffoldConfig;
}

export interface PhotoCounts {
  total: number;
  ready: number;
  pending: number;
  error: number;
}

export interface EventSummary {
  id: string;
  rootPath: string;
  name: string;
  slug: string;
  description?: string | null;
  status: string;
  presetId: string;
  subjectLabel: string;
  folderLevels: string[];
  orderCode: string;
  stationCode: string;
  taxRateBasisPoints: number;
  photoCounts: PhotoCounts;
  subjectCount: number;
  orderCount: number;
  createdAt: string;
}

export interface LibraryEventEntry {
  root_path: string;
  event_id: string;
  name: string;
  last_opened_at: string;
  exists_on_disk: boolean;
}

export interface LibraryBundleOverview {
  rootPath: string;
  eventId: string;
  name: string;
  lastOpenedAt: string;
  existsOnDisk: boolean;
  orderCode: string | null;
  stationCode: string | null;
  photoCounts: PhotoCounts | null;
  subjectCount: number;
  orderCount: number;
  paidOrderCount: number;
  grossPaidCents: number;
  pendingFulfillmentCount: number;
  inspectError: string | null;
}

export interface LibraryTotals {
  totalEvents: number;
  onlineEvents: number;
  totalPhotos: number;
  readyPhotos: number;
  totalSubjects: number;
  totalOrders: number;
  paidOrders: number;
  grossPaidCents: number;
  pendingFulfillmentOrders: number;
}

export interface LibraryOverviewResponse {
  events: LibraryBundleOverview[];
  totals: LibraryTotals;
  duplicateOrderNumbers: string[];
}

export interface LanServerStatus {
  running: boolean;
  port: number;
  lanIp: string | null;
  lanUrl: string | null;
}

export interface CardReaderInfo {
  reader_id: string;
  usb_serial?: string | null;
  display_name: string;
  mount_point: string;
  volume_name: string;
  bus_protocol: string;
  is_internal: boolean;
  disk_id: string;
  file_count: number;
  folder_count: number;
}

export interface DirectoryEntry {
  name: string;
  path: string;
  is_directory: boolean;
  file_count: number;
}

export interface CopyProgressEvent {
  total: number;
  copied: number;
  current_file: string;
  percentage: number;
}

export interface ReaderConfig {
  readerId: string;
  displayName: string;
  customLabel?: string | null;
  groupName?: string | null;
  sortOrder: number;
  destination: string;
  photographer: string;
  cameraBrand: string;
  cameraFolderPath: string;
  renamePrefix: string;
  autoRename: boolean;
  fileRenamePrefix?: string | null;
  requireSubfolder: boolean;
  currentOrder: number;
}

export interface ReaderLayoutUpdate {
  readerId: string;
  groupName: string | null;
  sortOrder: number;
}

export interface IngestResult {
  batchId: string;
  copiedCount: number;
  queuedCount: number;
}

export interface ScanResult {
  photosFound: number;
  photosNew: number;
  photosSkipped: number;
  photosRemoved: number;
}

export interface WatcherStatus {
  enabled: boolean;
  watching: boolean;
  watchedRoot: string | null;
  lastSyncAt: string | null;
  lastAdded: number;
  lastRemoved: number;
  totalAdded: number;
  totalRemoved: number;
  lastError: string | null;
}

export interface WatcherSyncEvent {
  eventRoot: string;
  added: number;
  removed: number;
  queued: number;
  syncedAt: string;
}

export interface ProcessingMetrics {
  engine: string;
  workerThreads: number;
  inFlight: number;
  queued: number;
  completed: number;
  errors: number;
  photosPerSec: number;
  etaSeconds: number | null;
  avgDecodePreviewMs: number;
  avgEncodePreviewMs: number;
  avgThumbMs: number;
  avgTotalMs: number;
  avgDbFlushMs: number;
}

export interface AvatarMetrics {
  engine: string;
  queued: number;
  inFlight: number;
  completed: number;
  errors: number;
  totalWithPhotos: number;
  cachedAvatars: number;
}

export interface SubjectRecord {
  id: string;
  number: string | null;
  firstName: string;
  lastName: string | null;
  displayName: string;
  groupLabel: string | null;
  teamName: string | null;
  photoCount: number;
}

export interface RosterEntryInput {
  number: string | null;
  firstName: string;
  lastName: string | null;
  groupLabel: string | null;
  teamName: string | null;
  externalId: string | null;
}

export interface ParsedRosterFile {
  filePath: string;
  filename: string;
  defaultGroupLabel: string | null;
  entries: RosterEntryInput[];
  warnings: string[];
}

export interface ImportRosterResult {
  importedNew: number;
  updated: number;
  autoMatchedPhotos: number;
}

export interface UnassignedFolder {
  folderPath: string;
  parentPath: string;
  leafFolder: string;
  photoCount: number;
  samplePhotoIds: string[];
}

export interface FolderParentSummary {
  parentPath: string;
  folderCount: number;
  photoCount: number;
}

export interface UnassignedFoldersResponse {
  parents: FolderParentSummary[];
  folders: UnassignedFolder[];
  totalUnassignedPhotos: number;
}

export interface AssignFolderResult {
  photosUpdated: number;
  newFolderPath: string;
  renamedOnDisk: boolean;
}

export interface FolderItem {
  folderPath: string;
  parentPath: string;
  leafFolder: string;
  photoCount: number;
  unassignedCount: number;
  assignedSubjectId: string | null;
  assignedSubjectName: string | null;
  assignedSubjectNumber: string | null;
  isMixed: boolean;
  samplePhotoIds: string[];
}

export interface FolderLocationSummary {
  parentPath: string;
  folderCount: number;
  photoCount: number;
  unassignedFolderCount: number;
  unassignedPhotoCount: number;
}

export interface ListFoldersResponse {
  locations: FolderLocationSummary[];
  folders: FolderItem[];
  totalUnassignedPhotos: number;
  totalUnassignedFolders: number;
  detectedSessionGroup: string | null;
  detectedGroupPrefix: string | null;
  sameGroupSubjectIds: string[];
  subjectSamplePhotos: Record<string, string>;
  sectionLabel: string | null;
  sectionMatchedSubjects: Record<string, string>;
}

export interface IngestBatchSummary {
  id: string;
  sequenceNum: number;
  readerId: string | null;
  readerName: string | null;
  volumeName: string | null;
  photographer: string | null;
  destinationRelPath: string;
  totalPhotos: number;
  readyPhotos: number;
  pendingPhotos: number;
  totalFolders: number;
  unassignedFolders: number;
  unassignedPhotos: number;
  createdAt: string;
}

export interface FolderReassignmentInput {
  folderPath: string;
  targetSubjectId: string | null;
}

export interface FolderChangeResultItem {
  oldFolderPath: string;
  newFolderPath: string;
  targetSubjectId: string | null;
  targetSubjectName: string | null;
  photosUpdated: number;
  action: "renamed" | "swapped" | "merged" | "reassigned" | "unassigned" | string;
}

export interface BatchReassignResult {
  foldersUpdated: number;
  photosUpdated: number;
  renamedOnDisk: boolean;
  changes: FolderChangeResultItem[];
}

export type RemoveFolderMode = "trash" | "delete_disk" | "db_only";

export interface RemoveFolderResult {
  folderPath: string;
  photosRemoved: number;
  mode: RemoveFolderMode;
  trashedTo: string | null;
}

export interface ProductRecord {
  id: string;
  productType: "single_digital" | "full_digital" | "usb_addon" | "print" | "custom" | string;
  name: string;
  size: string | null;
  priceCents: number;
  isAvailable: boolean;
  displayOrder: number;
}

export interface ShippingAddress {
  street: string;
  street2: string | null;
  city: string;
  state: string;
  zip: string;
}

export interface OrderItemRecord {
  id: string;
  orderId: string;
  productId: string | null;
  subjectId: string | null;
  subjectDisplayName: string | null;
  photoId: string | null;
  photoFilename: string | null;
  productType: string;
  productName: string;
  productSize: string | null;
  quantity: number;
  unitPriceCents: number;
  lineTotalCents: number;
  fulfillmentStatus: "pending" | "fulfilled" | string;
  fulfilledAt: string | null;
}

export interface OrderRecord {
  id: string;
  orderNumber: string;
  stationCode: string;
  customerName: string;
  customerEmail: string | null;
  customerPhone: string | null;
  shippingAddress: ShippingAddress | null;
  subtotalCents: number;
  taxRateBasisPoints: number;
  taxCents: number;
  totalCents: number;
  paymentStatus: "pending" | "paid" | "refunded" | string;
  paymentMethod: string | null;
  paymentReference: string | null;
  paidAt: string | null;
  notes: string | null;
  createdAt: string;
  items: OrderItemRecord[];
  squarePosUrl: string;
}

export interface OrderStats {
  totalOrders: number;
  pendingPaymentCount: number;
  paidCount: number;
  refundedCount: number;
  pendingUsbCount: number;
  pendingFulfillmentCount: number;
  grossPaidCents: number;
  subtotalPaidCents: number;
  taxPaidCents: number;
  pendingRevenueCents: number;
}

export interface OrderListResponse {
  stats: OrderStats;
  orders: OrderRecord[];
}

export interface UsbDriveInfo {
  diskId: string;
  volumeName: string;
  mountPoint: string;
  busProtocol: string;
  totalBytes: number;
  freeBytes: number;
  hasDcim: boolean;
}

export interface UsbSubjectBreakdown {
  subjectLabel: string;
  photoCount: number;
  totalBytes: number;
  sections: [string, number][];
}

export interface UsbOrderPlan {
  orderId: string;
  orderNumber: string;
  totalPhotos: number;
  totalBytes: number;
  subjects: UsbSubjectBreakdown[];
  missingFiles: number;
}

export interface UsbCopyProgressEvent {
  totalFiles: number;
  copiedFiles: number;
  totalBytes: number;
  copiedBytes: number;
  currentFile: string;
  mbPerSec: number;
  percentage: number;
}

export interface UsbCopyResult {
  destinationDir: string;
  filesCopied: number;
  bytesCopied: number;
  elapsedSecs: number;
  mbPerSec: number;
  ejected: boolean;
  ejectWarning: string | null;
  itemsMarkedFulfilled: number;
}

export interface CsvExportResult {
  outputPath: string;
  eventCount: number;
  orderCount: number;
  itemRowCount: number;
  skippedEvents: string[];
}
