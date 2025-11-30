export interface SessionConfig {
  eventSlug: string;
  photographer: string;
  gym: string;
  sessionNumber: string;
  destinationRoot: string;
  cardReaderPath: string;
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
