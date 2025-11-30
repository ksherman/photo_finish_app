export interface SessionConfig {
  destination: string;
  photographer: string;
  currentOrder: number;
}

export interface VolumeInfo {
  name: string;
  path: string;
  is_removable: boolean;
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

export interface CardReaderInfo {
  reader_id: string;
  display_name: string;
  mount_point: string;
  volume_name: string;
  bus_protocol: string;
  is_internal: boolean;
  disk_id: string;
  file_count: number;
}

export interface Competitor {
  id: string;
  competitorNumber: string;
  firstName: string;
  lastName: string;
  displayName: string;
  teamName: string;
}

export interface FolderRename {
  originalName: string;
  originalPath: string;
  newName: string;
  photoCount: number;
  competitorId?: string;
}
