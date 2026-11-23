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

export interface IngestResult {
  batchId: string;
  copiedCount: number;
  queuedCount: number;
}

export interface ScanResult {
  photosFound: number;
  photosNew: number;
  photosSkipped: number;
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
