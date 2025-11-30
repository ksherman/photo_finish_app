# Event Photo Management System
## Product Definition Document

**Version:** 1.4  
**Date:** November 22, 2025  
**Last Updated:** November 22, 2025  
**Product Owner:** [Your Name]  
**Lead Engineer:** [Your Name]  

**Status:** Requirements Complete - Ready for Development

**Major Components:** Phoenix Server + Tauri Ingestion App + Web Interfaces + Ordering System

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Problem Statement](#problem-statement)
3. [Solution Overview](#solution-overview)
4. [Technical Architecture](#technical-architecture)
5. [Core Features](#core-features)
6. [User Personas](#user-personas)
7. [Use Cases](#use-cases)
8. [Non-Functional Requirements](#non-functional-requirements)
9. [Database Design](#database-design)
10. [Photo Ingestion Workflow](#photo-ingestion-workflow)
11. [Open Questions](#open-questions)
12. [Success Metrics](#success-metrics)
13. [Project Timeline](#project-timeline)
14. [Appendix](#appendix)

---

## Executive Summary

The Event Photo Management System is a local-network photo browsing and management solution designed for live events such as gymnastics meets, dance recitals, and sports competitions. The system enables photographers to ingest and organize photos in real-time during an event, while participants and families can browse and view photos on multiple viewing stations throughout the venue.

### Key Differentiators

- **On-site, local network operation** with no internet dependency
- **Real-time photo availability** during the event via file system watching
- **Flexible hierarchical organization** adaptable to any event structure
- **Dual interface:** admin for management, viewer for public browsing
- **High-performance architecture** leveraging NAS storage and Phoenix LiveView

---

## Problem Statement

### Current Challenges

- Event photographers struggle with real-time photo distribution at venues
- Participants and families want immediate access to photos during and after performances
- Internet-dependent solutions are unreliable in venues with poor connectivity
- Existing photo management tools are either too complex or too rigid for event-specific needs
- Manual organization and distribution of hundreds or thousands of photos is time-consuming
- Large volumes of photos (approaching 1TB per weekend event) make cloud solutions impractical

---

## Solution Overview

The Event Photo Management System provides a complete on-site solution with two primary components:

### 1. Admin Interface

A file-explorer style interface for photographers and event staff to:

- Monitor automatic photo discovery from NAS storage
- Manually scan for new photos on-demand
- Organize photos into a flexible hierarchical structure
- Create and manage organizational units (gyms, sessions, events, competitors, etc.)
- Verify photo ingestion and correct misplaced items
- Monitor system status and viewing station activity

### 2. Viewer Interface

A public-facing browsing interface for participants and families to:

- Navigate the event hierarchy via breadcrumb-style cards
- Search for specific competitors by name
- View full-resolution photos in a gallery interface
- See live updates as new photos are discovered and processed

---

## Technical Architecture

### System Components

The system consists of **three main components**:

1. **Tauri Ingestion Application** (Desktop - Rust + VueJS)
   - Memory card reading and file copying
   - Barcode scanner integration
   - Folder renaming interface
   - Phoenix API communication

2. **Phoenix Application Server** (Elixir + LiveView)
   - File discovery and processing
   - Database management
   - Admin web interface
   - Viewer web interface
   - Background job processing

3. **NAS File Storage**
   - Photo storage (originals, previews, thumbnails)
   - Shared network access
   - Optional HTTP serving (nginx)

### Technology Stack

**Ingestion Application (Tauri):**
- Rust (Tauri framework backend)
- VueJS 3 (frontend UI)
- Tailwind CSS
- HTTP client for Phoenix API

**Backend:**
- Elixir 1.17+
- Phoenix 1.7+ with Phoenix LiveView
- Ecto + PostgreSQL
- Image processing: `vix` (libvips) or `mogrify` (ImageMagick)
- File watching: `file_system` library

**Frontend:**
- Phoenix LiveView (primary UI framework)
- Tailwind CSS
- Alpine.js for JavaScript interactions (as needed)
- Note: Product owner has VueJS experience, but LiveView preferred for:
  - Reduced complexity (less client-side state management)
  - Real-time features built-in
  - Faster development for this use case
  - VueJS components can be integrated if specific interactions require it

**Storage:**
- PostgreSQL for metadata and hierarchy
- NAS/local filesystem for photos and thumbnails
- Optional: nginx on NAS for direct file serving

**Background Processing:**
- Oban for job queue management
- Parallel thumbnail generation (leveraging multi-core processors)
- File system watching and ingestion pipeline
- Oban Web for monitoring (optional)

**Deployment:**
- Mix release for standalone executable
- Runs on Mac/Linux/Windows laptop or dedicated server
- Optional: Docker container for easier deployment

### Architecture Diagram

```
┌─────────────────────────────────────────────────┐
│      Tauri Ingestion App (Rust + Vue)           │
│      (Desktop Application)                       │
│                                                  │
│  - Memory card reader access                    │
│  - Barcode scanner input                        │
│  - File copying to NAS                          │
│  - Folder renaming                              │
│  - API communication with Phoenix               │
└────────────────┬────────────────────────────────┘
                 │
                 │ HTTP API
                 │
┌────────────────┴────────────────────────────────┐
│              NAS / File Server                   │
│                                                  │
│  /photos/ (originals)                           │
│  /previews/ (watermarked 1280px)                │
│  /thumbnails/ (400px)                           │
│                                                  │
│  HTTP Server (nginx) on :8080 (optional)        │
└───────────────┬──────────────────────────────────┘
                │
                │ (mounted or HTTP)
                │
┌───────────────┴──────────────────────────────────┐
│           Phoenix Application Server             │
│                                                   │
│  ┌────────────────────────────────────────────┐ │
│  │ Ingestion API                              │ │
│  │  - Receive ingestion notifications         │ │
│  │  - Provide roster data                     │ │
│  └────────────────────────────────────────────┘ │
│                                                   │
│  ┌────────────────────────────────────────────┐ │
│  │ File Discovery System                       │ │
│  │  - FileWatcher (GenServer)                  │ │
│  │  - Manual Scanner                           │ │
│  │  - Ingestion Pipeline                       │ │
│  └────────────────────────────────────────────┘ │
│                                                   │
│  ┌────────────────────────────────────────────┐ │
│  │ Web Interface (Phoenix LiveView)           │ │
│  │  - Admin UI (auth required)                 │ │
│  │  - Viewer UI (public)                       │ │
│  └────────────────────────────────────────────┘ │
│                                                   │
│  ┌────────────────────────────────────────────┐ │
│  │ PostgreSQL Database                         │ │
│  │  - Event metadata                           │ │
│  │  - Hierarchy structure                      │ │
│  │  - Photo records (paths/URLs)               │ │
│  │  - Ingestion tracking                       │ │
│  └────────────────────────────────────────────┘ │
│                                                   │
│  Port: 4000                                      │
└───────────────┬──────────────────────────────────┘
                │
        ┌───────┴────────┐
        │                │
   ┌────┴─────┐    ┌─────┴─────┐
   │ Viewer   │    │  Viewer   │
   │ Station  │    │  Station  │
   │ (Tablet) │    │  (Tablet) │
   └──────────┘    └───────────┘
```

### Why This Stack?

**Phoenix/Elixir Advantages:**
- **Real-time built-in:** Phoenix PubSub and LiveView handle live updates naturally
- **Concurrency:** BEAM VM excels at handling many concurrent viewing stations
- **Fault tolerance:** Supervisors ensure system reliability during events
- **Performance:** Fast enough to handle thumbnail generation and serving

**Web-Based UI Advantages:**
- **Zero installation** on viewing stations - just open browser
- **Cross-platform:** Works on tablets, phones, laptops without separate builds
- **Instant updates:** Refresh browser, no app reinstall needed
- **Single codebase:** One UI works everywhere

**NAS Storage Advantages:**
- **Fast ingestion:** Direct file copy is 10x faster than HTTP upload
- **No bandwidth bottleneck:** Phoenix doesn't handle upload traffic
- **Scalable storage:** Just add more drives to NAS
- **Backup-friendly:** NAS handles RAID, snapshots, etc.

### File Serving Strategy

**Option A: Phoenix serves all files** (Simpler)
- NAS mounted to Phoenix server (e.g., `/mnt/nas`)
- Phoenix serves via `send_file/3`
- All traffic through Phoenix
- Easier setup, good for MVP

**Option B: NAS serves files directly** (Recommended for scale)
- NAS runs HTTP server (nginx)
- Phoenix stores URLs in database: `http://nas.local:8080/photos/...`
- Viewing stations fetch directly from NAS
- Better performance, offloads Phoenix

**Hybrid Recommendation:**
- Start with Option A (simpler)
- Move to Option B if performance requires it

### Deployment Model

**Phase 1: Manual Setup (MVP)**
```bash
# Build release
MIX_ENV=prod mix release

# At event venue:
./bin/event_photos start
# Server runs on http://localhost:4000
# Viewing stations connect to http://server-ip:4000
```

**Configuration:**
```elixir
# config/runtime.exs
config :event_photos,
  photo_root: System.get_env("PHOTO_ROOT") || "/mnt/nas/photos",
  thumbnail_root: System.get_env("THUMBNAIL_ROOT") || "/mnt/nas/thumbnails",
  nas_http_base: System.get_env("NAS_HTTP_BASE") # optional
```

**Phase 2: Packaged Deployment**
- Docker container with docker-compose
- Includes PostgreSQL
- Simple setup script
- Environment-based configuration

---

## Core Features

### Admin Interface Features

#### Event Setup
- Create new event with name, date, and description
- Define hierarchy levels (e.g., Gym → Session → Event → Flight → Competitor)
- Configure level names and properties
- **Import competitor roster** (optional but recommended)
  - **CSV format:** Columns include competitor_id, first_name, last_name, team, etc.
  - **PDF rotation sheets:** Semi-manual extraction of format `{ID} {FIRST} {LAST_INITIAL}`
    - Example: "1022 Kevin S" → ID: 1022, Name: Kevin S.
    - May require manual data entry or assisted extraction
- Pre-create organizational structure before event
  - Optionally create all hierarchy nodes (gyms, sessions, events, flights)
  - Or allow dynamic creation during photo ingestion

#### Photo Discovery & Ingestion
- **Automatic file watching** - detects new photos as they're copied to NAS
- **Manual scan trigger** - on-demand scan for new photos
- Real-time ingestion status monitoring
- Thumbnail generation progress
- Error handling for corrupt/invalid files
- Duplicate detection

#### Photo Management
- File browser with grid and list views
- Navigate hierarchy via tree view sidebar
- Breadcrumb navigation
- View photo details (EXIF data, file info)
- Move photos between categories (if misplaced)
- Delete photos with confirmation
- Search and filter photos
- View ingestion history and logs

#### System Monitoring
- View connected viewing stations
- Photo count statistics by hierarchy level
- Storage usage monitoring
- Recent activity log
- Ingestion pipeline status
- Error/warning notifications

### Viewer Interface Features

#### Navigation
- Breadcrumb card-based navigation through hierarchy
- Visual cards showing each organizational level
- Photo count indicators at each level
- Back navigation through breadcrumb trail
- Responsive design for all screen sizes

#### Search
- Search bar for competitor names
- Auto-complete suggestions
- Direct navigation to competitor photos from search results
- Fuzzy matching for typos

#### Photo Viewing
- Grid view of all photos for selected competitor/category
- Click to view full-size photo in lightbox/modal
- Next/previous navigation in lightbox
- Zoom and pan controls
- Touch-optimized for tablets
- Keyboard shortcuts for navigation

#### Real-time Updates
- Automatic refresh when new photos are discovered
- New photo indicators/notifications
- No page refresh required
- Phoenix PubSub broadcasts updates to all viewers

---

## User Personas

### Event Photographer / Administrator

**Profile:**
- Professional or semi-professional event photographer
- Takes hundreds to thousands of photos during an event
- Needs to make photos available quickly during or immediately after the event
- Wants to provide immediate value to customers (participants/families)
- May have assistant photographers or staff helping with organization

**Goals:**
- Fast, reliable photo ingestion
- Minimal manual organization needed
- Real-time availability for customers
- Easy verification that all photos are accessible

**Pain Points:**
- Manual upload is too slow for large volumes
- Complex UIs slow down workflow
- Unreliable systems cause customer dissatisfaction

### Event Organizer

**Profile:**
- Runs gymnastics meets, dance recitals, or sports competitions
- Wants to provide enhanced experience for participants and families
- May manage the setup and configuration of the system
- Needs system to work reliably without technical intervention

**Goals:**
- Easy setup at venue
- Reliable operation during event
- Positive feedback from participants/families
- Minimal technical issues

**Pain Points:**
- Internet dependency at venues
- Complex setup procedures
- System failures during events

### Participant / Competitor

**Profile:**
- Gymnast, dancer, or athlete competing in the event
- Wants to see photos of their performance immediately
- May be a child or teenager with varying technical proficiency
- Expects simple, intuitive interface

**Goals:**
- Find their photos quickly
- View high-quality images
- Share with family/friends

**Pain Points:**
- Complex navigation
- Can't find their photos
- Slow loading times

### Parent / Family Member

**Profile:**
- Attending event to watch family member compete
- Wants to view and potentially purchase photos
- May be using system on personal mobile device or venue kiosks
- Values quick access to see their child's photos

**Goals:**
- Find child's photos easily
- View and save favorites
- Purchase prints (future feature)

**Pain Points:**
- Too many photos to browse through
- Difficult search/navigation
- Photos not available until after event

---

## Use Cases

### Use Case 1: Event Setup

**Actor:** Event Organizer or Photographer

**Preconditions:** Server and NAS are set up and connected

**Flow:**
1. Create new event with name and date
2. Define hierarchy structure (levels and names)
   - Example: Gym → Session → Event → Flight → Competitor
3. Optionally import competitor roster (CSV)
4. Create organizational folders on NAS matching structure
   - Can be done manually or via admin interface
5. Configure file watcher paths
6. Set up viewing stations on local network

**Postconditions:** System is ready to accept photos

---

### Use Case 2: Photo Ingestion During Event

**Actor:** Photographer or Event Staff

**Preconditions:** Event is set up, hierarchy exists

**Flow:**
1. Photographer captures photos during event
2. At break/intermission, photographer transfers photos from memory card to computer
3. Staff copies photos to appropriate folder on NAS:
   ```
   /photos/gym-a/session-1/floor/a-flight/sarah-johnson/*.jpg
   ```
4. File watcher detects new files (or admin triggers manual scan)
5. System processes each photo:
   - Creates database record
   - Extracts EXIF metadata
   - Generates thumbnail
   - Marks as ready
6. Phoenix broadcasts update via PubSub
7. Viewing stations automatically show new photos
8. Admin can verify ingestion in admin interface

**Postconditions:** Photos are available to viewers within seconds

**Alternative Flows:**
- **Batch import:** Copy entire memory card to staging area, then move to final location
- **Error handling:** System flags corrupt/invalid files for review
- **Wrong folder:** Admin can move photos to correct location via UI

---

### Use Case 3: Browse Photos as Participant

**Actor:** Participant or Family Member

**Preconditions:** Photos have been ingested for competitor

**Flow:**
1. Access viewer interface on viewing station or personal device
2. Either:
   - **Option A:** Search for competitor name ("Sarah Johnson")
   - **Option B:** Navigate hierarchy (Gym A → Session 1 → Floor → A Flight → Sarah Johnson)
3. View grid of all photos for competitor
4. Click photo to view full size in lightbox
5. Use next/previous arrows to browse through photos
6. Zoom/pan to see details
7. (Future) Favorite/bookmark photos for later

**Postconditions:** User has viewed photos

---

### Use Case 4: Correct Misplaced Photos

**Actor:** Photographer or Admin

**Preconditions:** Some photos were copied to wrong folder

**Flow:**
1. Admin notices photos in wrong category (via monitoring or reports)
2. Navigate to misplaced photos in admin interface
3. Select photos to move
4. Choose correct destination in hierarchy
5. Confirm move operation
6. System updates database records
7. If photos were already viewed, they now appear in correct location
8. Old location no longer shows these photos

**Postconditions:** Photos are in correct location

---

### Use Case 5: Handle Ingestion Errors

**Actor:** Admin

**Preconditions:** Some files failed to process

**Flow:**
1. Admin sees error notification in dashboard
2. Navigate to "Ingestion Errors" view
3. Review list of failed files with error messages:
   - Corrupt JPEG
   - Unsupported format
   - File locked/in use
   - Insufficient permissions
4. For each error:
   - Retry processing (if transient error)
   - Delete file (if truly corrupt)
   - Convert format (if unsupported)
   - Fix permissions (if access issue)
5. Mark errors as resolved

**Postconditions:** All errors are handled

---

## Non-Functional Requirements

### Performance

- Photo discovery and database record creation: < 1 second per image
- Thumbnail generation: < 2 seconds per image
- Thumbnail loading in viewer: < 500ms
- Full image loading on local network: < 2 seconds
- Support for 10,000+ photos per event
- Support for 20+ concurrent viewing stations without degradation
- Hierarchy navigation: < 200ms response time

### Reliability

- System uptime of 99.9% during event hours (max 5 minutes downtime per 8-hour event)
- Graceful handling of network interruptions
- No data loss during file ingestion
- Automatic recovery from server restarts
- File watcher resilience to missed events
- Database transaction safety for concurrent operations

### Usability

- Viewer interface requires no training - intuitive for first-time users
- Admin interface learnable in < 30 minutes
- Responsive design for tablets (10"+) and larger screens
- Touch-optimized for kiosk stations
- Clear error messages with actionable guidance
- Visual feedback for long-running operations (progress bars, spinners)

### Scalability

- Handle events with 100-1,000+ participants
- Support up to **100,000+ photos per event** (1,000 competitors × 100 photos each)
- Accommodate 15 concurrent viewers without performance degradation
- Database queries remain fast even with 100K+ records (proper indexing critical)
- Thumbnail generation throughput: Must parallelize to handle 100K photos
- Photo grid pagination/virtual scrolling for competitors with 100+ photos

### Performance Optimization Strategy

Given the scale (100K+ photos), optimization is critical:

**1. Database Performance**
```sql
-- Critical indexes for 100K+ records
CREATE INDEX idx_photos_event_node ON photos(event_id, node_id);
CREATE INDEX idx_photos_filename ON photos(filename);
CREATE INDEX idx_photos_status_event ON photos(status, event_id);
CREATE INDEX idx_hierarchy_path ON hierarchy_nodes(event_id, parent_id, level_number);
```

**2. Thumbnail Generation**
- **Sequential**: 100K photos × 2 sec = 55 hours ❌
- **Parallel (4 cores)**: 100K photos ÷ 4 = ~14 hours ✅
- **Strategy**: Continuous background processing during event
- Consider pre-processing overnight if photos available early

**3. Photo Grid Display**
- **Virtual scrolling** for competitors with 100 photos
- Load thumbnails on-demand (lazy loading)
- Implement pagination: 20-50 photos per page
- Cache thumbnails aggressively in browser

**4. Real-time Updates**
- Phoenix PubSub can easily handle 15 connections
- **Selective broadcasting**: Only to nodes affected by new photos
- Don't broadcast every photo to every viewer
- Example: Broadcast to "photos:competitor-id" topic, not global

**5. File Serving**
- Consider CDN-like caching for thumbnails
- nginx on NAS for direct static file serving (offload Phoenix)
- HTTP/2 for parallel thumbnail loading

**Target Metrics:**
- Thumbnail loading: < 100ms for grid of 50 thumbnails
- Full photo loading: < 500ms
- Search response: < 200ms across 1,000 competitors
- Photo ingestion: Process 1,000 photos in < 30 minutes (background)

### Security

- Admin interface protected by password authentication
- Role-based access control (admin vs. viewer)
- Secure file storage with proper permissions
- Protection against malicious file uploads (validation, size limits)
- XSS and CSRF protection (Phoenix defaults)
- SQL injection prevention (Ecto parameterized queries)

### Maintainability

- Well-documented codebase
- Comprehensive test coverage (unit, integration, E2E)
- Logging for troubleshooting
- Configuration via environment variables
- Database migrations for schema changes

---

## Database Design

### Schema Overview

The system uses a flexible hierarchical structure that adapts to any event type:

```
events
  └── hierarchy_levels (defines structure)
  └── hierarchy_nodes (actual organizational units)
      └── photos
```

### Tables

#### `events`
Top-level container for an entire photo collection.

```sql
CREATE TABLE events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    event_date DATE,
    status VARCHAR(50) DEFAULT 'active', -- active, archived, deleted
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_events_status ON events(status);
CREATE INDEX idx_events_date ON events(event_date);
```

**Example:**
- "State Gymnastics Championship 2025"
- "Spring Dance Recital 2025"

---

#### `hierarchy_levels`
Defines the structure of the hierarchy for each event.

```sql
CREATE TABLE hierarchy_levels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    level_number INTEGER NOT NULL, -- 1, 2, 3, 4, 5
    level_name VARCHAR(100) NOT NULL, -- "Gym", "Session", "Event"
    level_name_plural VARCHAR(100), -- "Gyms", "Sessions", "Events"
    is_required BOOLEAN DEFAULT true,
    allow_photos BOOLEAN DEFAULT false, -- Can photos be attached at this level?
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(event_id, level_number)
);

CREATE INDEX idx_hierarchy_levels_event ON hierarchy_levels(event_id);
```

**Example for Gymnastics:**
```
event_id: xyz, level: 1, name: "Gym", allow_photos: false
event_id: xyz, level: 2, name: "Session", allow_photos: false
event_id: xyz, level: 3, name: "Event", allow_photos: false
event_id: xyz, level: 4, name: "Flight", allow_photos: false
event_id: xyz, level: 5, name: "Competitor", allow_photos: true
```

---

#### `hierarchy_nodes`
The actual organizational units (folders/categories).

```sql
CREATE TABLE hierarchy_nodes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    parent_id UUID REFERENCES hierarchy_nodes(id) ON DELETE CASCADE,
    level_number INTEGER NOT NULL,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL, -- URL-friendly version
    display_order INTEGER DEFAULT 0,
    metadata JSONB, -- Flexible custom attributes
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_hierarchy_nodes_event ON hierarchy_nodes(event_id);
CREATE INDEX idx_hierarchy_nodes_parent ON hierarchy_nodes(parent_id);
CREATE INDEX idx_hierarchy_nodes_level ON hierarchy_nodes(event_id, level_number);
CREATE INDEX idx_hierarchy_nodes_slug ON hierarchy_nodes(event_id, slug);
```

**Example Data:**
```
id: n1, event: xyz, parent: null, level: 1, name: "Gym A", slug: "gym-a"
id: n2, event: xyz, parent: n1, level: 2, name: "Session 1", slug: "session-1"
id: n3, event: xyz, parent: n2, level: 3, name: "Floor", slug: "floor"
id: n4, event: xyz, parent: n3, level: 4, name: "A Flight", slug: "a-flight"
id: n5, event: xyz, parent: n4, level: 5, name: "Sarah Johnson", slug: "sarah-johnson"
```

---

#### `photos`
The actual photo files and metadata.

```sql
CREATE TABLE photos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    node_id UUID REFERENCES hierarchy_nodes(id) ON DELETE CASCADE,
    
    -- File paths (two-phase approach)
    ingestion_path VARCHAR(1000) NOT NULL, -- Original path when discovered
    current_path VARCHAR(1000), -- Current path (after moves/finalization)
    previous_path VARCHAR(1000), -- Previous path (for audit trail)
    preview_path VARCHAR(1000), -- Watermarked preview image
    thumbnail_path VARCHAR(1000), -- Small thumbnail
    
    -- Optional: URLs if served by NAS HTTP
    file_url VARCHAR(1000),
    preview_url VARCHAR(1000),
    thumbnail_url VARCHAR(1000),
    
    -- File information
    filename VARCHAR(255) NOT NULL,
    original_filename VARCHAR(255),
    file_size_bytes BIGINT,
    mime_type VARCHAR(100),
    width INTEGER,
    height INTEGER,
    
    -- Ingestion tracking
    photographer VARCHAR(100), -- Photographer name/initials
    order_number VARCHAR(50), -- Memory card order sequence
    source_folder VARCHAR(255), -- Original folder from card
    
    -- Timestamps
    captured_at TIMESTAMP, -- From EXIF
    discovered_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    processed_at TIMESTAMP, -- When preview/thumbnail generated
    finalized_at TIMESTAMP, -- When moved to final location
    moved_at TIMESTAMP, -- Last time file was moved
    moved_by UUID, -- Admin user who moved it
    
    -- Processing status
    status VARCHAR(50) DEFAULT 'discovered', 
    -- Values: discovered, processing, ready, finalized, error, duplicate
    error_message TEXT,
    
    -- Additional metadata
    exif_data JSONB, -- Full EXIF dump
    metadata JSONB -- Custom tags, flags, etc.
);

CREATE INDEX idx_photos_event ON photos(event_id);
CREATE INDEX idx_photos_node ON photos(node_id);
CREATE INDEX idx_photos_status ON photos(status);
CREATE INDEX idx_photos_captured ON photos(captured_at);
CREATE INDEX idx_photos_filename ON photos(filename);
CREATE INDEX idx_photos_photographer ON photos(photographer);
CREATE INDEX idx_photos_order ON photos(order_number);
```

**Photo Status Values:**
- `discovered`: Found by file watcher, not yet processed
- `processing`: Currently generating preview/thumbnail
- `ready`: Available for viewing (preview/thumbnail generated)
- `finalized`: Moved to final archival structure
- `error`: Failed processing (with error_message)
- `duplicate`: Already imported (same hash or filename)

---

#### `competitors` (Enhanced Roster Management)
Dedicated table for competitor/participant information beyond hierarchy nodes.

```sql
CREATE TABLE competitors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    node_id UUID REFERENCES hierarchy_nodes(id) ON DELETE SET NULL,
    
    -- Identification
    competitor_meet_id VARCHAR(50) NOT NULL, -- From roster: e.g., "1022"
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    
    -- Additional info
    team_name VARCHAR(255),
    level VARCHAR(50), -- Gymnastics level (5, 7, etc.)
    age_group VARCHAR(50), -- Junior, Senior, etc.
    
    -- Contact (optional, future)
    email VARCHAR(255),
    phone VARCHAR(50),
    
    -- Status
    is_active BOOLEAN DEFAULT true,
    
    -- Metadata
    metadata JSONB, -- Flexible for event-specific fields
    
    -- Timestamps
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP, -- Soft delete
    
    UNIQUE(event_id, competitor_meet_id)
);

CREATE INDEX idx_competitors_event ON competitors(event_id);
CREATE INDEX idx_competitors_node ON competitors(node_id);
CREATE INDEX idx_competitors_meet_id ON competitors(event_id, competitor_meet_id);
CREATE INDEX idx_competitors_name ON competitors(event_id, last_name, first_name);
CREATE INDEX idx_competitors_team ON competitors(event_id, team_name);
CREATE INDEX idx_competitors_active ON competitors(event_id, is_active) WHERE deleted_at IS NULL;
```

**Purpose:**
- Store pre-event roster information
- Rich competitor data beyond just hierarchy nodes
- Enable search by ID, name, or team
- Track which competitors have photos

**Relationship to Hierarchy:**
- `node_id` links competitor record to hierarchy node (when photos exist)
- Node may be created automatically when first photo is ingested
- Or admin can pre-create nodes from roster

**Example Data:**
```
id: c1, meet_id: "1022", name: "Kevin Smith", team: "Bay Area", node_id: node-123
id: c2, meet_id: "1023", name: "Sarah Johnson", team: "Elite", node_id: node-456
```

**Roster Import:**
```elixir
def import_roster_csv(file, event_id) do
  file
  |> File.stream!()
  |> CSV.decode!(headers: true)
  |> Enum.map(fn row ->
    %{
      event_id: event_id,
      competitor_meet_id: row["competitor_id"],
      first_name: row["first_name"],
      last_name: row["last_name"],
      team_name: row["team"],
      level: row["level"],
      age_group: row["age_group"]
    }
  end)
  |> Enum.each(&create_competitor/1)
end
```

---
For additional categorization beyond hierarchy.

```sql
CREATE TABLE photo_tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    photo_id UUID REFERENCES photos(id) ON DELETE CASCADE,
    tag_name VARCHAR(100) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(photo_id, tag_name)
);

CREATE INDEX idx_photo_tags_photo ON photo_tags(photo_id);
CREATE INDEX idx_photo_tags_name ON photo_tags(tag_name);
```

**Example Tags:** "spotlight", "action-shot", "medal-ceremony", "warmup"

---

### Key Queries

#### Get Full Hierarchy Path for a Node
```sql
WITH RECURSIVE node_path AS (
    -- Base case: start with the target node
    SELECT id, parent_id, level_number, name, slug, 1 as depth
    FROM hierarchy_nodes
    WHERE id = $1
    
    UNION ALL
    
    -- Recursive case: get parent nodes
    SELECT hn.id, hn.parent_id, hn.level_number, hn.name, hn.slug, np.depth + 1
    FROM hierarchy_nodes hn
    INNER JOIN node_path np ON hn.id = np.parent_id
)
SELECT * FROM node_path ORDER BY depth DESC;
```

#### Get All Photos for a Competitor (with hierarchy info)
```sql
SELECT 
    p.*,
    hn.name as competitor_name,
    hn.slug as competitor_slug
FROM photos p
JOIN hierarchy_nodes hn ON p.node_id = hn.id
WHERE hn.id = $1
  AND p.status = 'ready'
ORDER BY p.captured_at, p.filename;
```

#### Search for Competitor by Name
```sql
SELECT hn.*, hl.level_name, COUNT(p.id) as photo_count
FROM hierarchy_nodes hn
JOIN hierarchy_levels hl ON hn.event_id = hl.event_id 
    AND hn.level_number = hl.level_number
LEFT JOIN photos p ON p.node_id = hn.id AND p.status = 'ready'
WHERE hn.event_id = $1
  AND hl.allow_photos = true
  AND hn.name ILIKE $2
GROUP BY hn.id, hl.level_name
ORDER BY hn.name;
```

#### Get Photo Count by Node (including descendants)
```sql
WITH RECURSIVE node_tree AS (
    -- Start with target node
    SELECT id FROM hierarchy_nodes WHERE id = $1
    
    UNION ALL
    
    -- Get all descendants
    SELECT hn.id
    FROM hierarchy_nodes hn
    INNER JOIN node_tree nt ON hn.parent_id = nt.id
)
SELECT COUNT(*)
FROM photos p
WHERE p.node_id IN (SELECT id FROM node_tree)
  AND p.status = 'ready';
```

---

## Photo Ingestion Workflow

### Overview

Photos are ingested via direct file copy to NAS, not through HTTP upload. This is faster, more reliable, and handles the large volume (up to 1TB) more efficiently.

### Workflow Steps

```
1. Photographer captures photos on camera
2. Memory card handed to staff at event
3. Staff copies files to NAS:
   /photos/gym-a/session-1/floor/a-flight/sarah-johnson/*.jpg
4. Phoenix discovers files (automatically or via manual trigger)
5. For each discovered file:
   a. Create database record (status: discovered)
   b. Extract EXIF metadata
   c. Generate thumbnail
   d. Update status to 'ready'
   e. Broadcast update via PubSub
6. Viewers automatically show new photos
```

### File Discovery Mechanisms

The system supports two complementary discovery methods:

#### 1. Automatic File Watching (Real-time)

Uses the `file_system` Elixir library to watch for file system events.

**Implementation:**
```elixir
defmodule EventPhotos.PhotoWatcher do
  use GenServer
  require Logger
  
  def start_link(opts) do
    GenServer.start_link(__MODULE__, opts, name: __MODULE__)
  end
  
  def init(_opts) do
    photo_root = Application.get_env(:event_photos, :photo_root)
    
    {:ok, watcher_pid} = FileSystem.start_link(
      dirs: [photo_root],
      name: :photo_watcher
    )
    
    FileSystem.subscribe(watcher_pid)
    
    {:ok, %{watcher: watcher_pid, photo_root: photo_root}}
  end
  
  def handle_info({:file_event, _watcher_pid, {path, events}}, state) do
    if should_process?(path, events) do
      Task.start(fn -> ingest_photo(path) end)
    end
    {:noreply, state}
  end
  
  defp should_process?(path, events) do
    # Only process new/modified image files
    Enum.member?(events, :created) or Enum.member?(events, :modified)
    and is_image_file?(path)
    and not is_thumbnail?(path)
  end
end
```

**Pros:**
- Automatic, no manual trigger
- Near-instant availability (< 1 second detection)
- Great for live events

**Cons:**
- May miss files if watcher starts after bulk copy
- Can trigger on partial/temporary files
- More complex error handling

#### 2. Manual Scan (On-demand)

Admin can trigger a full directory scan via the UI.

**Implementation:**
```elixir
defmodule EventPhotos.PhotoScanner do
  def scan_directory(path, event_id) do
    path
    |> File.ls!()
    |> Enum.filter(&is_image_file?/1)
    |> Enum.reject(&already_ingested?(&1, event_id))
    |> Enum.map(&ingest_photo(&1, event_id))
  end
  
  def scan_with_progress(path, event_id) do
    # For admin UI - sends progress updates via PubSub
    files = discover_all_images(path)
    total = length(files)
    
    files
    |> Enum.with_index()
    |> Enum.each(fn {file, index} ->
      ingest_photo(file, event_id)
      progress = (index + 1) / total * 100
      broadcast_progress(event_id, progress)
    end)
  end
end
```

**Pros:**
- Reliable - won't miss files
- Full control over timing
- Can show progress bar in UI
- Good for bulk imports

**Cons:**
- Not automatic
- Requires manual trigger
- Slight delay

**Recommendation:** Use both!
- File watcher for automatic discovery during event
- Manual scan as backup and for pre-event bulk imports
- Admin UI shows: "Auto-scan: ✓ ON | Last scan: 2 min ago | [Scan Now]"

---

### File Path to Hierarchy Mapping

The system uses a **two-phase approach** to file organization:

#### Phase 1: Ingestion Path (Source Preservation)

Photos are initially copied to a structure that preserves source context:

```
/photos/{EVENT}/{PHOTOGRAPHER}/{GYM}/{SESSION}/{ORDER}/{SOURCE_FOLDERS}/*.jpg

Example:
/photos/st-valentines-meet/kds/gym-a/session-1/0001/EOS100/IMG_001.jpg
```

**Structure Breakdown:**
- `{EVENT}` - Event name/slug
- `{PHOTOGRAPHER}` - Photographer name/initials (e.g., "KDS")
- `{GYM}` - Gym identifier (e.g., "gym-a")
- `{SESSION}` - Session number (e.g., "session-1")
- `{ORDER}` - Order number for this batch (e.g., "0001", "0002")
  - Represents the sequence memory cards were received
  - Critical for troubleshooting - "What was on the 3rd card we got?"
- `{SOURCE_FOLDERS}` - Original folder structure from memory card
- `*.jpg` - Actual photo files

**Why This Structure:**
- **Traceability** - Can always trace back to source if something goes wrong
- **Order matters** - Knowing card sequence helps resolve issues
- **Flexible** - Doesn't force immediate categorization
- **Safe** - Preserves photographer's original organization

**Database Tracking:**
```elixir
# photos table
ingestion_path: "/photos/st-valentines-meet/kds/gym-a/session-1/0001/EOS100/IMG_001.jpg"
current_path: nil  # Will be set during finalization
photographer: "KDS"
order_number: "0001"
source_folder: "EOS100"
status: "discovered"
```

#### Phase 2: Finalization (Optional Reorganization)

After verification, photos can be reorganized for clean archival structure:

**Option A: By Competitor**
```
/photos/{EVENT}/{COMPETITOR_ID}/{EVENT_TYPE}/*.jpg

Example:
/photos/st-valentines-meet/0234/floor/IMG_001.jpg
/photos/st-valentines-meet/0234/beam/IMG_002.jpg
```

**Option B: Full Hierarchy**
```
/photos/{EVENT}/{GYM}/{SESSION}/{EVENT}/{FLIGHT}/{COMPETITOR}/*.jpg

Example:
/photos/st-valentines-meet/gym-a/session-1/floor/a-flight/sarah-johnson/IMG_001.jpg
```

**Finalization Process:**
1. Admin reviews ingested photos in UI
2. Corrects any misassignments (wrong competitor, etc.)
3. When session is complete and verified, triggers "Finalize"
4. System physically moves files to clean structure
5. Database updates `current_path` and marks as finalized

**Benefits:**
- **Happy path**: Clean, browsable archive structure
- **Problem path**: Can always reference ingestion path to debug
- **Flexibility**: Can reorganize without losing source information

#### Hybrid Parsing Strategy

**During Ingestion:**
```elixir
def parse_ingestion_path(path) do
  # /photos/{event}/{photographer}/{gym}/{session}/{order}/{source}/file.jpg
  parts = Path.split(path)
  
  %{
    event_slug: Enum.at(parts, 1),
    photographer: Enum.at(parts, 2),
    gym_slug: Enum.at(parts, 3),
    session_slug: Enum.at(parts, 4),
    order_number: Enum.at(parts, 5),
    source_folder: Enum.at(parts, 6),
    filename: List.last(parts)
  }
end

def ingest_from_path(path, event_id) do
  parsed = parse_ingestion_path(path)
  
  # Look up or create hierarchy nodes
  with {:ok, gym} <- find_or_create_node(event_id, 1, parsed.gym_slug),
       {:ok, session} <- find_or_create_node(event_id, 2, parsed.session_slug, gym.id) do
    
    # At this point, we may not know flight/competitor yet
    # Photo is assigned to session level, pending manual categorization
    create_photo(%{
      event_id: event_id,
      node_id: session.id,
      ingestion_path: path,
      photographer: parsed.photographer,
      order_number: parsed.order_number,
      status: "pending_categorization"
    })
  end
end
```

**Manual Categorization:**
- Admin sees photos grouped by session/order
- Assigns photos to correct competitor
- System moves file to finalized structure
- Updates database with current_path

**After Finalization:**
```elixir
def finalize_photo(photo, target_node_id) do
  # Generate clean path
  target_path = generate_finalized_path(photo, target_node_id)
  
  # Physical file move
  :ok = File.rename(photo.ingestion_path, target_path)
  
  # Update database
  update_photo(photo, %{
    node_id: target_node_id,
    current_path: target_path,
    status: "finalized",
    finalized_at: DateTime.utc_now()
  })
end
```

#### Correction Workflow

**Scenario 1: Wrong Competitor Name**
```
UI: Admin sees "julie-smith" folder has photos
Action: Admin renames to "julie-jones"
System: Physically renames folder, updates all photo records
```

**Scenario 2: Mixed Photos in One Folder**
```
UI: Admin sees folder with photos of multiple competitors
Action: Admin selects photos, assigns each to correct competitor
System: Moves individual files to correct folders, creates folders if needed
```

**Scenario 3: Wrong Session/Flight**
```
UI: Admin notices competitor in wrong flight
Action: Admin moves competitor folder to correct flight
System: Physically moves entire folder, updates hierarchy
```

**Implementation:**
```elixir
def move_photos(photo_ids, target_node_id) do
  Enum.each(photo_ids, fn photo_id ->
    photo = get_photo(photo_id)
    new_path = generate_path_for_node(photo, target_node_id)
    
    # Ensure target directory exists
    :ok = File.mkdir_p(Path.dirname(new_path))
    
    # Move file
    :ok = File.rename(photo.current_path || photo.ingestion_path, new_path)
    
    # Track movement in database
    update_photo(photo, %{
      node_id: target_node_id,
      previous_path: photo.current_path || photo.ingestion_path,
      current_path: new_path,
      moved_at: DateTime.utc_now(),
      moved_by: current_admin_user_id()
    })
  end)
  
  # Broadcast update to viewers
  broadcast_photos_moved(photo_ids, target_node_id)
end
```

#### Practical Workflow Example

**Scenario:** Session 1 of Gym A is happening. Photographer hands off first memory card.

**Step 1: Initial Copy**
Staff copies memory card to NAS:
```
/photos/st-valentines-meet/kds/gym-a/session-1/0001/EOS100/
  IMG_8234.jpg
  IMG_8235.jpg
  IMG_8236.jpg
  ...
```

**Step 2: Auto-Discovery**
- File watcher detects new files
- System parses path and creates records:
  - Event: "St Valentines Meet"
  - Photographer: "KDS"
  - Gym: "Gym A"
  - Session: "Session 1"
  - Order: "0001"
  - Source: "EOS100"
- Photos assigned to Session level (not yet to specific competitors)
- Status: "pending_categorization"

**Step 3: Processing**
- Generate watermarked previews → `/previews/st-valentines-meet/{photo_id}.jpg`
- Generate thumbnails → `/thumbnails/st-valentines-meet/{photo_id}.jpg`
- Status: "ready"
- Viewers can now see photos (grouped by session)

**Step 4: Manual Categorization** (Admin UI)
- Admin views Session 1 photos
- Sees folder "EOS100" with 45 photos
- Reviews photos, notices they're from Floor event, A Flight
- Knows this is Sarah Johnson based on rotation schedule
- Admin actions:
  1. Select all 45 photos
  2. Assign to: Gym A → Session 1 → Floor → A Flight → Sarah Johnson
  3. Confirm

**Step 5: Photo Assignment**
- System moves files:
  ```
  FROM: /photos/st-valentines-meet/kds/gym-a/session-1/0001/EOS100/IMG_8234.jpg
  TO:   /photos/st-valentines-meet/gym-a/session-1/floor/a-flight/sarah-johnson/IMG_8234.jpg
  ```
- Database updated:
  - `node_id` = Sarah Johnson's node
  - `current_path` = new location
  - `ingestion_path` = original (preserved)
- Viewers now see photos under Sarah Johnson

**Step 6: Correction** (if needed)
- Staff realizes: "Wait, those were from B Flight, not A Flight!"
- Admin actions:
  1. Navigate to Sarah Johnson (currently under A Flight)
  2. Select competitor folder
  3. "Move to..." → B Flight
  4. Confirm

- System moves entire folder:
  ```
  FROM: /photos/.../a-flight/sarah-johnson/
  TO:   /photos/.../b-flight/sarah-johnson/
  ```

**Step 7: Finalization** (after event, optional)
- Admin reviews all Session 1 photos, verifies correctness
- Triggers "Finalize Session 1"
- System reorganizes to archival structure:
  ```
  FROM: /photos/st-valentines-meet/gym-a/session-1/floor/a-flight/sarah-johnson/IMG_8234.jpg
  TO:   /photos/st-valentines-meet/1022-sarah-johnson/floor/IMG_8234.jpg
  ```
- Easier for browsing when fulfilling orders
- Original ingestion path still in database for reference

**Result:**
- Photos discoverable within seconds of copy
- Flexible correction workflow
- Clean archival structure
- Full audit trail maintained

---

### Image Processing Pipeline

The system generates three versions of each photo:

1. **Original** - Untouched from camera, stored for orders/printing
2. **Watermarked Preview** - ~1280px, watermarked, for viewing
3. **Thumbnail** - Small (~400px), generated from watermarked preview

**Strategy:**
```elixir
def process_photo(photo) do
  # Step 1: Verify original is valid
  original_path = photo.file_path
  {:ok, original} = Vix.Vips.Image.new_from_file(original_path)
  
  # Step 2: Generate watermarked preview (1280px max dimension)
  preview_path = preview_path_for(photo)
  {:ok, preview} = Vix.Vips.Operation.thumbnail_image(original, 1280)
  {:ok, watermarked} = apply_watermark(preview, watermark_config())
  :ok = Vix.Vips.Image.write_to_file(watermarked, preview_path, quality: 90)
  
  # Step 3: Generate thumbnail from watermarked preview (400px max)
  thumbnail_path = thumbnail_path_for(photo)
  {:ok, thumbnail} = Vix.Vips.Operation.thumbnail_image(watermarked, 400)
  :ok = Vix.Vips.Image.write_to_file(thumbnail, thumbnail_path, quality: 85)
  
  # Update database with paths
  update_photo(photo, %{
    preview_path: preview_path,
    thumbnail_path: thumbnail_path,
    status: "ready",
    processed_at: DateTime.utc_now()
  })
end

defp apply_watermark(image, config) do
  # Watermark implementation - overlay text or logo
  # Position: bottom-right corner, semi-transparent
  # Text: photographer name, event name, or "PROOF"
  # TODO: Design watermark appearance
  {:ok, image} # Placeholder
end
```

**File Storage Structure:**
```
/mnt/nas/
  originals/
    {event}/
      {photographer}/
        {gym}/
          {session}/
            {order}/
              {source_folders}/
                *.jpg
  
  previews/
    {event}/
      {photo_id}.jpg  # Watermarked 1280px
  
  thumbnails/
    {event}/
      {photo_id}.jpg  # Small thumbnail
```

**Performance:**
- Process asynchronously via `Task.Supervisor`
- Concurrent processing (limit to CPU cores, e.g., 4)
- Generate thumbnail from preview (faster than from original)
- Store in separate directories for easy management

---

### Edge Cases & Error Handling

#### File System Issues

**Problem:** File is locked, being written, or in use

**Solution:**
- Wait and retry (with exponential backoff)
- After N retries, flag as error
- Admin can manually retry later

**Problem:** Insufficient permissions

**Solution:**
- Check permissions on startup
- Clear error message to admin
- Document required permissions in setup guide

#### Invalid/Corrupt Files

**Problem:** File is not a valid image or is corrupted

**Solution:**
- Catch exceptions during image loading
- Mark photo as error status with message
- Admin can review and delete
- Log for troubleshooting

#### Duplicate Files

**Problem:** Same file uploaded multiple times

**Detection:**
- Compare filename + file size
- Optional: Compare hash (slower but accurate)

**Solution:**
- Mark as duplicate status
- Admin can choose to keep or delete
- Optional: auto-delete duplicates

#### Missing Hierarchy Nodes

**Problem:** File path references non-existent nodes

**Solution Option A:** Auto-create missing nodes
- Parse path and create nodes as needed
- Useful for flexible setup

**Solution Option B:** Flag as error
- Require all nodes to exist before ingestion
- More strict, better data quality

**Recommendation:** Configurable option, default to auto-create

#### File Moved/Deleted After Discovery

**Problem:** File watcher detects file, but it's gone before processing

**Solution:**
- Check file existence before processing
- Mark as error if missing
- Clean up orphaned database records periodically

#### Large Files

**Problem:** Very large RAW files (30+ MB) slow down processing

**Solution:**
- Set reasonable size limit (e.g., 50 MB)
- Convert RAW to JPEG before copying to NAS (external tool)
- Or handle RAW separately with warning in UI

---

### Ingestion Pipeline Architecture

```elixir
defmodule EventPhotos.IngestionPipeline do
  @moduledoc """
  Manages the photo ingestion process from discovery to ready.
  """
  
  def ingest_photo(file_path, event_id) do
    with {:ok, photo} <- create_photo_record(file_path, event_id),
         {:ok, photo} <- extract_exif(photo),
         {:ok, photo} <- generate_thumbnail(photo),
         {:ok, photo} <- mark_ready(photo) do
      broadcast_new_photo(photo)
      {:ok, photo}
    else
      {:error, reason} = error ->
        handle_ingestion_error(file_path, reason)
        error
    end
  end
  
  defp create_photo_record(file_path, event_id) do
    node_id = determine_node_from_path(file_path, event_id)
    
    attrs = %{
      event_id: event_id,
      node_id: node_id,
      file_path: file_path,
      filename: Path.basename(file_path),
      status: "discovered"
    }
    
    Photos.create_photo(attrs)
  end
  
  defp extract_exif(photo) do
    # Use ExifTool or similar
    case Exiftool.read(photo.file_path) do
      {:ok, exif} ->
        Photos.update_photo(photo, %{
          exif_data: exif,
          captured_at: parse_datetime(exif["DateTimeOriginal"]),
          width: exif["ImageWidth"],
          height: exif["ImageHeight"]
        })
      {:error, _} ->
        # Non-fatal, continue without EXIF
        {:ok, photo}
    end
  end
  
  defp generate_thumbnail(photo) do
    # Mark as processing
    Photos.update_photo(photo, %{status: "processing"})
    
    # Generate thumbnail (implementation above)
    # ...
  end
  
  defp mark_ready(photo) do
    Photos.update_photo(photo, %{
      status: "ready",
      processed_at: DateTime.utc_now()
    })
  end
  
  defp broadcast_new_photo(photo) do
    Phoenix.PubSub.broadcast(
      EventPhotos.PubSub,
      "photos:#{photo.node_id}",
      {:new_photo, photo}
    )
  end
end
```

---

### Admin UI for Ingestion Monitoring

**Dashboard View:**
- Total photos discovered: 1,247
- Ready for viewing: 1,245
- Processing: 1
- Errors: 1
- Last scan: 2 minutes ago
- Auto-watch: ✓ Enabled
- [Scan Now] button

**Error Log View:**
- List of photos with errors
- Error message
- File path
- Timestamp
- Actions: [Retry] [Delete] [Ignore]

**Recent Activity Feed:**
- Real-time list of ingested photos
- Shows: filename, competitor, timestamp
- Scrolling list, last 100 items

---

## Ordering System

### Overview

The ordering system enables on-site photo sales during events. Parents browse photos at viewing stations, select products, and submit orders that are fulfilled after payment. This is the **primary revenue stream** for the photography business.

**Key Statistics:**
- ~70% of orders are USB drives (all photos for one competitor)
- ~30% are individual prints and specialty products  
- Most orders placed during the event at viewing stations
- Some post-event online sales (separate system, future integration)

### Current Manual Process (To Be Digitized)

**At Viewing Station:**
1. Parents browse photos on kiosk
2. Fill out paper order form  
3. Write down image filenames (e.g., IMG_8234.jpg)
4. Select products and sizes
5. Take form to payment desk
6. Pay via Square/cash
7. Receive order number for tracking

**At Office After Event:**
1. **Manual data entry:** All paper forms entered into spreadsheet
2. **USB fulfillment:** Copy photo files to USB drives (ASAP)
3. **Print fulfillment:** Download photos, send to print lab  
4. **Shipping:** Mail prints when received from lab (1 week later)

**Pain Points:**
- Time-consuming manual data entry
- Handwriting interpretation errors
- Difficult to track order status
- Hard to find specific photos from written notes
- No real-time inventory or revenue tracking

---

### Digital Ordering - MVP Scope

**Phase 1 Focus: USB Drive Orders**

The MVP prioritizes USB drive orders (70% of business) with basic print support:

**Included:**
- Add photos to cart while browsing
- USB drive product (all photos for competitor)
- Individual print orders (select photo + size)
- Digital checkout form
- Order number generation
- Admin order tracking
- Payment recording (manual - Square/cash/check)
- USB fulfillment (file list for copying)
- Print fulfillment (download high-res photos)

**Explicitly Deferred:**
- Collage builder (manual creation workflow)
- Custom photo text/borders (manual)
- Square POS integration (auto-payment sync)
- Email receipts (manual for MVP)
- Print lab API integration (manual send)
- Inventory tracking
- Post-event online ordering portal

---

### Database Schema - Orders

**Note:** This adds to the existing database schema documented earlier.

```sql
-- Orders table
CREATE TABLE orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    competitor_id UUID REFERENCES competitors(id), -- Optional: which athlete
    
    -- Order identification  
    order_number VARCHAR(50) UNIQUE NOT NULL, -- "STV-0793" (event code + number)
    
    -- Customer information
    customer_name VARCHAR(255) NOT NULL,
    customer_email VARCHAR(255),
    customer_phone VARCHAR(50),
    athlete_name VARCHAR(255), -- For easy reference
    athlete_number VARCHAR(50), -- Meet ID like "891"
    
    -- Financial
    subtotal_cents INTEGER NOT NULL,
    tax_rate DECIMAL(5,2) DEFAULT 8.5, -- 8.5% tax rate
    tax_cents INTEGER NOT NULL,
    total_cents INTEGER NOT NULL,
    
    -- Status tracking
    status VARCHAR(50) NOT NULL DEFAULT 'pending_payment',
    -- pending_payment, paid, fulfilling, ready_for_pickup, shipped, complete, cancelled
    
    -- Payment details
    payment_method VARCHAR(50), -- square, cash, check  
    payment_reference VARCHAR(255), -- Square transaction ID, check number
    paid_at TIMESTAMP,
    
    -- Fulfillment tracking
    fulfillment_started_at TIMESTAMP,
    fulfilled_at TIMESTAMP,
    shipped_at TIMESTAMP,
    tracking_number VARCHAR(255),
    
    -- Metadata
    notes TEXT,
    internal_notes TEXT, -- Staff notes, not shown to customer
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    INDEX idx_orders_event (event_id),
    INDEX idx_orders_number (order_number),
    INDEX idx_orders_status (event_id, status),
    INDEX idx_orders_competitor (competitor_id),
    INDEX idx_orders_created (event_id, created_at DESC)
);

-- Order items (line items in an order)
CREATE TABLE order_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID REFERENCES orders(id) ON DELETE CASCADE,
    
    -- Product identification
    product_type VARCHAR(50) NOT NULL,
    -- usb_drive, print, collage_3photo, custom_photo
    
    product_size VARCHAR(50),
    -- NULL for USB, or: 5x7, 8x10, 11x14, 16x20, 24x30, poster
    
    -- Photo reference
    photo_id UUID REFERENCES photos(id) ON DELETE SET NULL,
    -- NULL for USB drive (all photos), UUID for specific photos
    
    -- Pricing
    unit_price_cents INTEGER NOT NULL,
    quantity INTEGER DEFAULT 1,
    line_total_cents INTEGER NOT NULL,
    
    -- Special product details (for future collage/custom support)
    collage_photo_ids JSONB, -- Array of photo IDs: ["uuid1", "uuid2", "uuid3"]
    customization_details JSONB, -- Border, text, etc.
    
    -- Fulfillment status per item
    item_status VARCHAR(50) DEFAULT 'pending',
    -- pending, ready, printed, on_usb, shipped
    fulfilled_at TIMESTAMP,
    
    -- Metadata
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    INDEX idx_order_items_order (order_id),
    INDEX idx_order_items_photo (photo_id),
    INDEX idx_order_items_status (item_status),
    INDEX idx_order_items_type (product_type)
);

-- Product catalog (configurable pricing per event)
CREATE TABLE products (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID REFERENCES events(id) ON DELETE CASCADE,
    -- NULL event_id = default global pricing
    
    -- Product definition
    product_type VARCHAR(50) NOT NULL,
    product_name VARCHAR(255) NOT NULL, -- "USB Drive - All Photos"
    product_description TEXT,
    product_size VARCHAR(50), -- NULL for USB
    
    -- Pricing
    price_cents INTEGER NOT NULL,
    
    -- Display
    is_active BOOLEAN DEFAULT true,
    display_order INTEGER DEFAULT 0,
    featured BOOLEAN DEFAULT false,
    
    -- Metadata
    sku VARCHAR(100),
    metadata JSONB, -- Extra configuration options
    
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    
    UNIQUE(event_id, product_type, product_size),
    INDEX idx_products_event_active (event_id, is_active),
    INDEX idx_products_featured (event_id, featured) WHERE featured = true
);
```

**Default Product Seeding:**

```elixir
# Seed default product catalog
products = [
  %{type: "usb_drive", name: "USB Drive - All Photos", size: nil, price: 10000, order: 1},
  %{type: "print", name: "5x7 Print", size: "5x7", price: 1800, order: 10},
  %{type: "print", name: "8x10 Print", size: "8x10", price: 3000, order: 11},
  %{type: "print", name: "11x14 Print", size: "11x14", price: 4000, order: 12},
  %{type: "print", name: "16x20 Print", size: "16x20", price: 6500, order: 13},
  %{type: "collage_3photo", name: "3 Photo Collage - 8x10", size: "8x10", price: 6000, order: 20},
  %{type: "collage_3photo", name: "3 Photo Collage - 11x14", size: "11x14", price: 7000, order: 21},
  %{type: "custom_photo", name: "Custom Photo - 8x10", size: "8x10", price: 5500, order: 30}
]
```

---

### Viewer Interface - Shopping Cart

**Shopping cart integrated into existing photo gallery viewer:**

**Add to Cart Button:**
```html
<!-- Each photo in gallery gets cart button -->
<div class="photo-card">
  <img src={@photo.thumbnail_url} phx-click="view_photo" />
  <div class="actions">
    <button phx-click="add_to_cart" phx-value-photo-id={@photo.id}>
      🛒 Add to Cart
    </button>
  </div>
</div>

<!-- Floating cart indicator -->
<div class="cart-badge" phx-click="show_cart">
  🛒 Cart (3)
</div>
```

**Cart Modal/Slide-out (LiveView component):**
- Show selected photos
- Select product for each photo (USB or print size)
- Display running total with tax
- Checkout button → checkout form

**Checkout Form Fields:**
- Customer name (required)
- Phone (optional)
- Email (optional)
- Athlete name (pre-filled if browsing from competitor)
- Athlete number (pre-filled)
- Order notes (optional)

**Order Confirmation:**
- Display order number prominently
- Instructions to take number to payment desk
- Total amount due
- Option to print receipt

---

### Admin Order Management

**Orders Dashboard:**
- List all orders for event
- Filter by status (pending, paid, fulfilling, complete)
- Search by order number or customer name
- Quick stats (revenue, order count, avg order value)

**Order Detail View:**
- Customer information
- Products ordered (with photo previews)
- Financial summary (subtotal, tax, total)
- Payment status
- Mark as paid (record payment method)
- Fulfillment actions per item

**USB Fulfillment:**
```
When admin views USB drive order item:
1. Show list of all photo files for that competitor
2. Provide file paths for manual copying
3. Option to download as zip (future)
4. Mark as complete when USB ready
```

**Print Fulfillment:**
```
Batch process for print orders:
1. Select multiple print order items
2. Download high-res photos in bulk
3. Generate manifest CSV (order#, customer, photo, size)
4. Send to print lab manually (MVP)
5. Mark as sent to lab
```

---

### Key Features - Phase 1 (MVP)

**Viewer Interface:**
- [ ] Add to cart button on photos
- [ ] Shopping cart modal
- [ ] Product selection (USB drive, print sizes)
- [ ] Checkout form
- [ ] Order confirmation with order number
- [ ] Tax calculation (8.5% fixed)

**Admin Interface:**
- [ ] Orders list (dashboard)
- [ ] Order detail view
- [ ] Record payment (Square/cash/check)
- [ ] USB fulfillment (file list)
- [ ] Print fulfillment (batch download)
- [ ] Order search and filtering
- [ ] Print receipts

**Backend:**
- [ ] Orders database tables
- [ ] Order creation context
- [ ] Product catalog seeding
- [ ] Order number generation
- [ ] Tax calculation
- [ ] Photo file collection for USB
- [ ] High-res photo download for prints

---

### Implementation Notes

**Order Number Format:**
- Event code (3 letters from event name) + sequential number
- Example: "STV-0793" for St. Valentines Meet, order 793
- Unique per event

**Tax Calculation:**
- Fixed 8.5% rate for MVP
- Applied to subtotal
- Rounded to nearest cent

**USB Fulfillment (MVP):**
- Admin views list of file paths
- Staff manually copies to USB drive
- Mark as complete when done
- Future: Auto-generate zip or copy script

**Print Fulfillment (MVP):**
- Batch download high-res photos
- Generate CSV manifest
- Manual upload to print lab
- Track when sent and received
- Future: Print lab API integration

---

### Success Metrics

**Operational:**
- Reduce order entry time from 2 hours to < 15 minutes
- Zero transcription errors (100% accuracy)
- USB fulfillment < 10 minutes per order

**Customer:**
- Checkout in < 3 minutes
- Clear order tracking via order number

**Business:**
- Real-time revenue tracking during event
- Product sales analytics

---

## Open Questions

### 1. Authentication & Access Control ✅ DECIDED

**Decision:** Completely open viewing stations (no authentication)

**Rationale:**
- Public viewing at physical event venue
- Anyone can browse any photos
- Simpler UX, faster access
- Appropriate for venue kiosks

**Admin Authentication:**
- Basic HTTP auth or simple password
- Just needs minimal protection
- Single admin account for MVP

**Future Consideration:**
- Browser kiosk mode (Guided Access on iOS, etc.) to prevent users from exiting app
- This is outside application scope - handled by device configuration

---

### 2. Photo Privacy & Watermarking ✅ DECIDED

**Decision:** All photos publicly viewable, with watermarking

**Viewing Experience:**
- All photos visible to everyone at venue
- No restrictions by session/gym
- Participants can browse competitors' photos

**Watermarking:**
- Preview images (1280px) are watermarked
- Thumbnails are watermarked
- Originals stored without watermark (for orders/printing)
- **TODO:** Design watermark appearance (photographer logo, "PROOF", event name, etc.)

---

### 3. Data Persistence & Archival ✅ DECIDED

**Decision:** Out of scope for MVP

**Current Approach:**
- Each event may be a fresh database (or accumulate events)
- No automated archival process
- Data may be reset between events if needed

**Future Consideration:**
- Possible deployment to ordering website (post-event)
- Need to maintain event data for later orders
- Will be addressed in future features discussion

---

### 4. Multi-Event Support ✅ DECIDED

**Decision:** Single active event per deployment, but maintain event structure

**Implementation:**
- Top-level `events` table exists in schema
- Each event has unique ID and metadata
- System designed to support multiple events in database
- MVP: Only one active event at a time
- Physical deployment = one event at venue

**Why maintain event structure:**
- May accumulate events over time
- Prepares for future ordering website
- Clean data model

---

### 5. Photo Format Support ✅ DECIDED

**Decision:** JPEG only for MVP

**Rationale:**
- Photographers provide JPEGs for meets
- RAW files too large for storage/processing
- If RAW accidentally used, convert to JPEG before copying to NAS

**Implementation:**
- File validation: Only accept JPEG files
- Clear error messages if non-JPEG uploaded
- File extension check: `.jpg`, `.jpeg` (case-insensitive)

---

### 6. File Organization Strategy ✅ DECIDED

**Decision:** Two-phase approach with physical file moves

**Phase 1: Ingestion Path**
```
/photos/{EVENT}/{PHOTOGRAPHER}/{GYM}/{SESSION}/{ORDER}/{SOURCE_FOLDERS}/*.jpg
```
- Preserves source structure from memory cards
- Tracks order sequence for troubleshooting
- Initial categorization may be incomplete

**Phase 2: Finalization** (Optional)
```
/photos/{EVENT}/{COMPETITOR_ID}/{EVENT_TYPE}/*.jpg
```
- Clean structure for archival and ordering
- Triggered after session verification
- Physical file reorganization

**Correction Mechanism:**
- Admin UI allows photo reassignment
- Physical file moves when correcting
- Database tracks both ingestion and current paths
- Audit trail maintained

**Why Physical Moves:**
- Filesystem stays clean and browsable
- Easy to find photos for orders
- No confusion about file location
- Simple mental model

---

### 7. Hardware Recommendations ✅ DECIDED

**Decision:** High-level guidance only, detailed planning by product owner

**Server Hardware:**
- **Platform:** MacOS (Apple Silicon preferred)
  - M-series chips provide excellent performance
  - 10+ CPU threads for parallel processing
  - Portable and reliable
  - Easy to replicate setup with similar hardware
- **Specs (estimated):**
  - 16GB+ RAM recommended
  - 500GB+ SSD for OS and application
  - NAS for photo storage
  - Gigabit Ethernet connection preferred

**Viewing Stations:**
- **Form Factor:** Tablets preferred
- **Budget Target:** ~$300 per station
- **Key Requirements:** 
  - Bigger screens better (10"+ recommended)
  - Reliable WiFi or Ethernet connection
  - Modern web browser
  - Touch-friendly interface
- **Quantity:** 15 stations typical for events
- **Note:** Specific models and procurement handled by product owner

**Networking:**
- **Infrastructure:** Likely Ubiquiti Networks gear
- **Requirements:**
  - Gigabit Ethernet for server
  - Strong WiFi for viewing stations
  - Dedicated network for event (not public WiFi)
- **Note:** Detailed network planning handled by product owner

**NAS Storage:**
- **Capacity:** 2TB+ recommended (supports ~6 events at 325GB each)
- **Performance:** Gigabit Ethernet connection
- **Optional:** HTTP server (nginx) for direct file serving
- **Backup:** RAID configuration recommended for reliability

**Documentation Approach:**
- Keep hardware guide high-level in application docs
- Focus on requirements and interfaces
- Product owner maintains separate hardware procurement guide
- Application should be hardware-agnostic where possible

---

## Remaining Open Questions

### 8. Competitor Roster Import ✅ DECIDED

**Decision:** Pre-event import with full CRUD capabilities

**Timing:**
- Roster typically received before meet weekend
- Import before event setup when possible
- Must support editing during event if needed

**Operations Required:**
- **Bulk import** from CSV
- **Add** individual competitors manually
- **Edit** competitor details (name, ID, team)
- **Remove** competitors (soft delete preferred)
- **Rename** competitors (corrections)

**CSV Format (to be finalized):**
```csv
competitor_id,first_name,last_name,team,level,age_group
1022,Kevin,Smith,Bay Area Gymnastics,5,Junior
1023,Sarah,Johnson,Elite Tumbling,7,Senior
```

**PDF Rotation Sheet Handling:**
- Format: `{ID} {FIRST} {LAST_INITIAL}` (e.g., "1022 Kevin S")
- Semi-manual extraction or data entry
- Admin can bulk paste if needed

---

### 9. Watermark Design

**Question:** What should the watermark look like?

**Options:**
- Photographer logo/name
- Event name
- "PROOF" text
- Date
- Small corner vs. large diagonal
- Opacity level

**Need to Decide:**
- Design specifications
- Configurable per event/photographer?
- Static image overlay or generated text?

**Recommendation:** Start simple with configurable text, add logo support later

---

### 10. Finalization Timing ✅ DECIDED

**Decision:** Manual trigger when "everything went well"

**Workflow:**
- Admin reviews session/event after completion
- Verifies all photos correctly categorized
- Manually triggers "Finalize Session X"
- System reorganizes to archival structure

**Benefits:**
- Full control over when finalization happens
- No risk of premature reorganization
- Can finalize per session or entire event

**UI:**
- Per-session finalize button
- "Finalize All" button for entire event
- Confirmation dialog before reorganization
- Progress indicator during finalization

---

### 11. Photo Display in Viewer ✅ DECIDED

**Decision:** Sort by filename (capture sequence)

**Rationale:**
- Photos numbered sequentially by camera
- Maintains photographer's intended order
- Reliable and predictable

**Implementation:**
- Primary sort: filename (alphanumeric)
- Fallback: captured_at timestamp if available
- Admin cannot manually reorder (keeps it simple)

**Example:**
```
IMG_8234.jpg
IMG_8235.jpg
IMG_8236.jpg
IMG_8237.jpg
```

Displays in this order automatically.

---

### 12. Error Recovery

**Question:** How should we handle ingestion errors in UI?

**Scenarios:**
- Corrupt JPEG
- Missing EXIF data (non-critical)
- Thumbnail generation fails
- File locked/in use
- Duplicate detection

**Proposed Approach:**
- **Non-critical errors** (missing EXIF): Log but continue, mark photo as ready
- **Processing errors** (thumbnail fails): Retry 3x, then mark as error
- **Critical errors** (corrupt file): Mark as error immediately
- **Duplicates**: Mark as duplicate, show in separate view

**Admin UI:**
- Dashboard shows error count
- "Errors" tab lists all failed photos
- For each error:
  - Filename and path
  - Error message
  - Actions: [Retry] [Delete] [Ignore]
- Bulk actions: [Retry All] [Delete All]

**Need to Confirm:**
- Level of detail in error messages?
- Auto-retry mechanism or manual only?

---

### 13. Performance Thresholds ✅ DECIDED

**Scale Specifications:**

**Per Competitor:**
- Target: 20-40 photos per event
- Events covered: 2-3 (Beam, Floor, sometimes Bars)
- **Total: ~100 photos per competitor**

**Per Event:**
- Competitors: Up to 1,000+
- **Total photos: 100,000+ photos** (1,000 competitors × 100 photos)
- File size: ~3 MB per original, ~200 KB per preview, ~50 KB per thumbnail
- **Storage: ~300 GB originals + ~20 GB previews + ~5 GB thumbnails = ~325 GB total**

**Viewing Stations:**
- Typical: 15 viewing stations at venue
- Future: Mobile browsing from stands (Phase 2+)

**Performance Implications:**

1. **Database Indexing Critical**
   - Need efficient queries across 100K+ photo records
   - Indexes on: event_id, node_id, filename, status
   - Hierarchy path queries must be optimized

2. **Pagination Required**
   - Cannot show 100 photos per competitor in single view
   - Implement virtual scrolling or pagination
   - Load thumbnails lazily

3. **Thumbnail Generation**
   - 100K photos × 2 sec = 55 hours sequential
   - **Must parallelize** - with 4 cores: ~14 hours
   - Consider overnight processing or continuous generation during event

4. **Network Bandwidth**
   - 15 stations × multiple users browsing
   - Thumbnail size critical (target 50 KB each)
   - Consider image optimization (WebP format?)

5. **Real-time Updates**
   - Phoenix PubSub can handle 15 connections easily
   - Broadcast photo additions selectively (per node, not global)

**Optimization Priorities:**
- Efficient database queries (proper indexes)
- Fast thumbnail generation (parallel processing)
- Lazy loading in viewer UI
- Selective PubSub broadcasting

---

## All Open Questions Summary

| # | Question | Status | Decision |
|---|----------|--------|----------|
| 1 | Authentication | ✅ Decided | Open viewing, basic admin auth |
| 2 | Privacy & Watermarking | ✅ Decided | Public photos, watermarked previews |
| 3 | Data Archival | ✅ Decided | Out of scope for MVP |
| 4 | Multi-Event Support | ✅ Decided | Single active event, maintain structure |
| 5 | Photo Formats | ✅ Decided | JPEG only |
| 6 | File Organization | ✅ Decided | Two-phase with physical moves |
| 7 | Hardware | ✅ Decided | High-level guidance only |
| 8 | Roster Import | ✅ Decided | Pre-event CSV, full CRUD |
| 9 | Watermark Design | ⏳ Pending | Design TBD |
| 10 | Finalization Timing | ✅ Decided | Manual trigger |
| 11 | Photo Sorting | ✅ Decided | By filename |
| 12 | Error Recovery | ⏳ Pending | Approach defined, details TBD |
| 13 | Performance Scale | ✅ Decided | 100K+ photos, 15 stations |

---

## Success Metrics

### MVP Success Criteria

- Successfully deploy at **3+ live events**
- Handle **1,000+ photos per event** without performance issues
- Support **10+ concurrent viewers** without degradation
- **90%+ of users** can find their photos without assistance
- **Zero critical bugs** or system crashes during events
- Photographer can ingest and organize **100 photos in < 10 minutes**
- Photos available to viewers within **1 minute** of file copy to NAS

### User Satisfaction Metrics

- Photographer feedback: ease of use (1-5 scale, target 4+)
- Viewer feedback: ability to find photos (1-5 scale, target 4+)
- Viewer feedback: photo quality and load speed (1-5 scale, target 4+)
- Percentage of participants who successfully viewed their photos (target 80%+)

### Technical Metrics

- Average time from file copy to "ready" status: < 2 seconds per photo
- Thumbnail generation throughput: 50+ photos per minute
- Page load time for viewer interface: < 1 second
- Search response time: < 200ms
- Uptime during events: 99.9%+

### Business Metrics (Long-term)

- Repeat usage at subsequent events
- Word-of-mouth adoption by other photographers/organizers
- Increased photo sales or engagement vs. previous methods
- Reduction in post-event manual organization time

---

## Photo Ingestion Workflow

### Overview

Photo ingestion is handled by a **dedicated desktop application** (Tauri-based) that manages memory card reading, file copying, and metadata entry. This approach is necessary because web browsers cannot access USB devices or local file systems directly.

### Current Workflow (To Be Replicated)

**At the Event:**

1. **Photographer Setup**
   - Session begins with multiple groups and flights (e.g., 4 groups × 2 flights = 8 rotations)
   - Photographer has 8 memory cards and 8 envelopes (one per rotation)
   - Each rotation labeled: "Group 1A", "Group 1B", "Group 2A", etc.
   - Photographer stays at one event apparatus (Floor, Beam, Bars, Vault)

2. **During Competition**
   - Photographer captures each competitor
   - Creates new folder on memory card between competitors
   - Folders typically named by camera: "EOS100", "EOS101", "EOS102", etc.
   - Writes competition order on envelope (if not fixed ahead of time)
   - Notes any issues on envelope (missed competitor, empty folders, etc.)
   - Places completed memory card in envelope after rotation

3. **Card Collection**
   - Staff periodically collects envelopes with memory cards
   - Brings them to ingestion station

**At Ingestion Station:**

1. **Session Setup** (one-time per session)
   - Set destination root path: `/event/Gym A/Session 3`
   - Configure folder renaming pattern: `EOS101` → `Gymnast 01`
   - Choose card reader drive letter: `L:/`
   - Load roster file: `1022 Kevin S\n1023 Sarah J\n...`

2. **Per Card Workflow**
   - Insert memory card into reader
   - Scan envelope barcode → Enters: `Group 1A/Beam`
   - Barcode input auto-populates rotation/event fields
   - Full destination: `/event/Gym A/Session 3/Group 1A/Beam/{ORDER}/`
   - Click "Download" → Files copied to NAS
   - Review copied folders in UI
   - Rename folders using roster: `Gymnast 01` → `1022 Kevin S`
   - Mark card as complete

3. **Result**
   - Photos organized in structured folders
   - Order of competition preserved
   - Metadata tracked (photographer, order number, envelope info)
   - Ready for Phoenix file watcher to discover

### Why This Workflow Works

**Checkpoints at Each Stage:**
- Memory card organization preserves source structure
- Envelope notes track any issues
- Order numbers maintain ingestion sequence
- Folder structure provides troubleshooting context

**Handles Edge Cases:**
- Empty folders (photographer tested shot, no keeper)
- Missed competitors (noted on envelope)
- Double-shooting photographers
- Out-of-order ingestion

### Architecture: Desktop App + Phoenix Server

```
┌─────────────────────────────────────────────────┐
│         Tauri Ingestion Application             │
│         (Desktop - Rust + VueJS)                │
│                                                 │
│  ┌──────────────────────────────────────────┐  │
│  │ VueJS Frontend                            │  │
│  │  - Memory card reader selection           │  │
│  │  - Barcode scanner input                  │  │
│  │  - File copy progress                     │  │
│  │  - Folder renaming interface              │  │
│  │  - Roster management                      │  │
│  └──────────────────────────────────────────┘  │
│                                                 │
│  ┌──────────────────────────────────────────┐  │
│  │ Rust Backend                              │  │
│  │  - File system access (USB/card readers)  │  │
│  │  - File copying with progress             │  │
│  │  - Barcode scanner event handling         │  │
│  │  - HTTP client (Phoenix API calls)        │  │
│  │  - Local configuration storage            │  │
│  └──────────────────────────────────────────┘  │
└────────────────┬────────────────────────────────┘
                 │
                 │ HTTP API (RESTful)
                 │ POST /api/ingestion/notify
                 │ GET  /api/roster
                 │
                 ▼
┌─────────────────────────────────────────────────┐
│         Phoenix Application Server              │
│                                                 │
│  ┌──────────────────────────────────────────┐  │
│  │ Ingestion API Endpoints                   │  │
│  │  - Receive ingestion notifications        │  │
│  │  - Provide roster data                    │  │
│  │  - Track ingestion progress               │  │
│  └──────────────────────────────────────────┘  │
│                                                 │
│  ┌──────────────────────────────────────────┐  │
│  │ File Watcher                              │  │
│  │  - Monitors NAS for new files             │  │
│  │  - Triggers processing pipeline           │  │
│  └──────────────────────────────────────────┘  │
│                                                 │
│  ┌──────────────────────────────────────────┐  │
│  │ Processing Pipeline (Oban)                │  │
│  │  - Create database records                │  │
│  │  - Extract EXIF metadata                  │  │
│  │  - Generate thumbnails & previews         │  │
│  └──────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

---

## Tauri Ingestion Application Specification

### Technology Stack

**Frontend:**
- VueJS 3 (Composition API)
- Tailwind CSS for styling
- Pinia for state management (if needed)

**Backend:**
- Rust (Tauri framework)
- `tokio` for async file operations
- `reqwest` for HTTP client
- `serde` for JSON serialization

**Platform:**
- Primary: macOS (Apple Silicon)
- Potential: Windows support

---

### Core Features

#### 1. Session Configuration

**UI Elements:**
- Root destination path selector (browse for folder)
- Photographer name input
- Card reader selection (dropdown of mounted volumes)
- Roster file loader (from Phoenix API or local .txt file)

**Storage:**
- Save session config locally (per event)
- Remember last used settings
- Quick session switching

**Example Config:**
```json
{
  "event_id": "uuid",
  "root_path": "/Volumes/NAS/photos/st-valentines-meet",
  "photographer": "KDS",
  "card_reader": "/Volumes/EOS_DIGITAL",
  "roster": [
    {"id": "1022", "name": "Kevin S"},
    {"id": "1023", "name": "Sarah J"}
  ]
}
```

#### 2. Card Reader Management

**Features:**
- Auto-detect mounted volumes
- Filter for memory card volumes (CF, SD, etc.)
- Multiple reader support (one per photographer)
- Reader assignment persistence

**UI:**
- Dropdown showing available volumes
- "Refresh" button to detect new cards
- Status indicator (card present/absent)

#### 3. Barcode/Envelope Input

**Workflow:**
- Text input field with focus on load
- Barcode scanner acts as keyboard (enters text + Enter)
- Parse format: `Group 1A/Beam` or `Group 3B/Floor`
- Auto-populate destination path preview

**Parsing Logic:**
```
Input: "Group 1A/Beam"
Parse to:
  - Group: "1A"  
  - Event: "Beam"
  - Destination: {root}/Group 1A/Beam/{order}/
```

**Manual Override:**
- Allow manual text entry (no barcode scanner)
- Validation of format
- Clear button to reset

#### 4. File Copy Operation

**Process:**
1. Verify source (memory card path exists)
2. Generate unique order number (auto-increment)
3. Create destination folder structure
4. Copy files preserving folder structure
5. Show progress (files copied / total, MB transferred)
6. Verify copy completed successfully
7. Notify Phoenix API

**UI:**
- "Copy Files" button (enabled when card + envelope ready)
- Progress bar with file count and size
- Current file being copied
- Elapsed time / estimated remaining
- Cancel operation option
- Success/error notification

**Error Handling:**
- Insufficient disk space
- Card ejected during copy
- Read errors
- Destination path issues

#### 5. Folder Renaming Interface

**Display:**
- List view of copied folders
- Original name → New name mapping
- Roster matching suggestions

**Workflow:**
1. Show all folders from memory card copy
   - Original: `EOS100`, `EOS101`, `EOS102`
2. For each folder, show rename options:
   - Auto-suggest from roster order
   - Manual selection from roster dropdown
   - Manual text entry
3. Bulk operations:
   - Auto-assign all (in order)
   - Clear all assignments
   - Skip empty folders

**Example UI:**
```
Folder Renaming
───────────────────────────────────────
Source Folder    → New Name         Photos
───────────────────────────────────────
EOS100          → [1022 Kevin S ▼]    12
EOS101          → [1023 Sarah J ▼]    15  
EOS102          → [1024 Emma W  ▼]    18
EOS103          → [Skip (empty)]       0
───────────────────────────────────────
[Auto-Assign]  [Apply All]  [Cancel]
```

**Implementation:**
- Actually rename folders on filesystem
- Update Phoenix via API with renamed paths
- Confirmation before applying

#### 6. Phoenix API Integration

**Endpoints Used:**

```
GET  /api/roster/:event_id
  → Returns competitor roster

POST /api/ingestion/notify
  → Notify Phoenix of completed copy
  Body: {
    event_id,
    photographer,
    envelope_code,
    order_number,
    source_path,
    destination_path,
    file_count,
    total_size_bytes
  }

POST /api/ingestion/rename
  → Report folder renames
  Body: {
    renames: [
      {original: "EOS100", new: "1022-kevin-s", photos: 12}
    ]
  }
```

**Error Handling:**
- Offline mode (Phoenix unavailable)
- Queue API calls for later
- Retry logic with exponential backoff
- Visual indication of sync status

#### 7. Status & Monitoring

**Dashboard View:**
- Cards ingested today: 24
- Total photos copied: 1,847
- Current session: Gym A / Session 3
- Last ingested: Group 2B/Floor (2 min ago)
- Phoenix sync status: ✓ Connected

**History Log:**
- Timestamp, envelope code, file count, status
- Filter by photographer, session, status
- Export log to CSV

---

### UI Mockup Structure

**Main Window (Single Screen):**

```
┌────────────────────────────────────────────────┐
│  Photo Ingestion Station        [Minimize] [×] │
├────────────────────────────────────────────────┤
│                                                │
│  Session: Gym A / Session 3                   │
│  Photographer: KDS              [Change Setup] │
│                                                │
├────────────────────────────────────────────────┤
│  Card Reader                                   │
│  ┌──────────────────────────────────────────┐ │
│  │ /Volumes/EOS_DIGITAL            [Detect] │ │
│  └──────────────────────────────────────────┘ │
│  Status: ● Card Ready (1,234 files, 3.2 GB)  │
│                                                │
│  Envelope Barcode / Code                      │
│  ┌──────────────────────────────────────────┐ │
│  │ Group 1A/Beam_____________        [Clear] │ │
│  └──────────────────────────────────────────┘ │
│                                                │
│  Destination:                                  │
│  /nas/st-valentines/KDS/Gym A/Session 3/       │
│  Group 1A/Beam/0001/                          │
│                                                │
│  ┌──────────────────────────────────────────┐ │
│  │        [Copy Files to Server]             │ │
│  └──────────────────────────────────────────┘ │
│                                                │
├────────────────────────────────────────────────┤
│  Copy Progress                                 │
│  ███████████████░░░░░░░  756 / 1234 files     │
│  Copying: IMG_8234.jpg (3.2 MB)               │
│  Elapsed: 02:15  Remaining: ~01:30            │
│                                                │
├────────────────────────────────────────────────┤
│  After Copy: Rename Folders                   │
│  [Show Renaming Interface]                    │
│                                                │
├────────────────────────────────────────────────┤
│  Status: Ready                  ✓ Connected    │
│  Cards Today: 8  Photos: 1,847                │
└────────────────────────────────────────────────┘
```

---

### Development Plan

#### Phase 1: MVP (Core Functionality)
**Estimated: 1-2 weeks**

- [ ] Basic Tauri app structure
- [ ] Session configuration UI
- [ ] Card reader detection
- [ ] Manual envelope code entry (no barcode yet)
- [ ] File copy with progress
- [ ] Basic Phoenix API notification
- [ ] Simple folder renaming UI

**Deliverable:** Functional ingestion without barcode scanner

#### Phase 2: Enhanced Workflow
**Estimated: 1 week**

- [ ] Barcode scanner input handling
- [ ] Roster loading from Phoenix API
- [ ] Auto-suggest folder renaming
- [ ] Bulk rename operations
- [ ] Error handling and retry logic
- [ ] Offline mode support

**Deliverable:** Full workflow with barcode support

#### Phase 3: Polish & Optimization
**Estimated: 1 week**

- [ ] Keyboard shortcuts
- [ ] Session history and logging
- [ ] Multi-card reader support
- [ ] Performance optimization
- [ ] Testing with real data
- [ ] Documentation

**Deliverable:** Production-ready application

---

### Technical Considerations

**File Copy Performance:**
- Use async I/O for responsiveness
- Chunk-based copying for progress updates
- Preserve file timestamps
- Verify checksums (optional)

**Memory Management:**
- Stream file copies (don't load entire files in memory)
- Clean up temp files
- Handle large directory listings

**Cross-Platform:**
- Start Mac-only, test on Apple Silicon
- Windows support if needed later
- Different path separators (/ vs \)
- Volume detection varies by OS

**Security:**
- No sensitive data stored
- API calls over HTTP (local network)
- Config stored in OS-appropriate location

---

### Integration with Phoenix

**Phoenix Side:**

```elixir
# API endpoint to receive ingestion notification
defmodule EventPhotosWeb.IngestionController do
  def notify(conn, params) do
    # Parse ingestion metadata
    with {:ok, attrs} <- validate_ingestion(params),
         {:ok, ingestion} <- create_ingestion_record(attrs) do
      
      # Trigger file watcher to pick up new files
      EventPhotos.FileWatcher.scan_path(attrs.destination_path)
      
      json(conn, %{status: "ok", ingestion_id: ingestion.id})
    end
  end
end
```

**Ingestion Record:**
```elixir
schema "ingestions" do
  field :event_id, :binary_id
  field :photographer, :string
  field :envelope_code, :string
  field :order_number, :string
  field :source_path, :string
  field :destination_path, :string
  field :file_count, :integer
  field :total_size_bytes, :integer
  field :status, :string # pending, processing, complete, error
  
  timestamps()
end
```

**File Discovery:**
- Tauri app copies files → NAS
- Tauri app notifies Phoenix via API
- Phoenix file watcher scans new path
- Processing pipeline triggered (Oban jobs)
- Photos appear in viewer

---

### Alternative: Simplified Approach

If Tauri development proves too complex, fallback options:

**Option A: Shell Script + Web UI**
- Bash/AppleScript for file copying
- Web UI for folder renaming
- Less polished but functional

**Option B: Phoenix CLI Commands**
- Terminal-based ingestion
- Staff types commands at server
- Simple but not user-friendly

**Recommendation:** Stick with Tauri - worth the effort for a polished workflow

---

### Prototype Experience

**Prior Validation Completed:**
- Thumbnail generation with libvips tested at scale
- Performance validated on Apple Silicon (M-series chips)
- File watching mechanisms prototyped
- Real-world meet photos available for testing

**Key Findings:**
- libvips thumbnail generation is extremely fast on modern hardware
- Apple Silicon chips (10+ threads) can process large batches efficiently
- 100K photos processable in ~2 hours with parallel generation
- File system watching performs well for real-time ingestion

**Test Data Available:**
- Complete meet's worth of photos (~100K images)
- Realistic file sizes and naming conventions
- Actual hierarchy structure from real events

### Performance Expectations (Validated)

**Thumbnail Generation:**
- Modern MacBook Pro/Mini: 10+ threads available
- libvips processing: Very fast (< 1 sec per image with optimization)
- Parallel processing: 10 concurrent = ~10x speedup
- **Realistic estimate: 100K photos in 2-3 hours** (not 14 hours)

**Database Performance:**
- PostgreSQL handles 100K+ rows efficiently with proper indexes
- Hierarchy queries tested and optimized
- No performance concerns with proper schema design

**UI Responsiveness:**
- Phoenix LiveView handles 15 concurrent users easily
- Real-time updates via PubSub tested and working
- Pagination necessary but straightforward

### Monitoring Strategy (Pragmatic)

**Essential:**
- Oban Web interface for job monitoring
- Basic error logging and alerts
- Disk space monitoring (simple threshold checks)

**Not Necessary for MVP:**
- Complex APM tools
- Detailed performance metrics
- Real-time analytics dashboards

**Graceful Degradation:**
- Placeholder images if thumbnails not ready
- Loading states in UI
- Clear error messages when files missing
- Simple, not over-engineered

---

## Project Timeline

### Phase 1: Planning & Design (2-3 weeks)

- ✅ Finalize requirements and feature prioritization
- ✅ Complete technical architecture decisions
- ✅ Design database schema (mostly done)
- ⏳ Complete UX/UI design for both interfaces
  - Admin file browser mockups
  - Viewer navigation flow
  - Photo gallery and lightbox design
  - **Tauri ingestion app UI mockups**
- Create user stories for MVP features
- Set up development environment (Phoenix + Tauri)

**Deliverables:**
- Finalized product definition document ✅
- Technical architecture document ✅
- UI/UX mockups (Phoenix + Tauri apps)
- User stories backlog

---

### Phase 2: Core Development (8-10 weeks)

**Week 1-2: Foundation**
- Set up Phoenix application
- Configure PostgreSQL database
- Implement database schema and migrations
- Create Ecto schemas and contexts
- Basic authentication for admin
- Set up Tauri project structure

**Week 3-4: Tauri Ingestion App (MVP)**
- Card reader detection and selection
- Manual envelope code entry
- File copy with progress tracking
- Basic Phoenix API endpoints (notify, roster)
- Simple folder renaming UI
- Session configuration

**Week 5-6: Photo Processing Pipeline**
- File watcher implementation
- Manual scan functionality
- Ingestion pipeline (discover → process → ready)
- Thumbnail and preview generation
- Error handling and retry logic
- Oban job setup

**Week 7-8: Admin Interface**
- File browser UI (tree view + content area)
- Event and hierarchy management
- Photo management (view, move, delete)
- Ingestion monitoring dashboard
- Roster import from CSV
- System status views

**Week 9-10: Viewer Interface**
- Breadcrumb navigation
- Search functionality
- Photo grid with pagination/virtual scrolling
- Lightbox/modal view
- Real-time updates (PubSub)

**Deliverables:**
- Functional MVP with all core features
- Tauri ingestion app (basic functionality)
- Passing test suite
- Basic documentation

---

### Phase 3: Enhancement & Testing (3-4 weeks)

**Week 1-2: Tauri App Enhancement**
- Barcode scanner integration
- Roster-based auto-suggest for renaming
- Bulk rename operations
- Improved error handling
- Offline mode support
- History and logging

**Week 3: Testing**
- Internal testing with sample data (1,000+ photos)
- Test Tauri app with real memory cards
- Performance testing (concurrent users, large datasets)
- Network resilience testing
- Edge case testing (corrupt files, permission issues)
- Cross-browser/device testing

**Week 4: Refinement**
- Bug fixes based on testing
- Performance optimization (query tuning, caching)
- UI polish and responsiveness
- Error message improvements
- Loading states and progress indicators

**Documentation:**
- Setup guide for deployment
- Admin user guide (Phoenix + Tauri apps)
- Troubleshooting guide
- API documentation

**Deliverables:**
- Production-ready application (Phoenix + Tauri)
- Comprehensive documentation
- Deployment guide

---

### Phase 4: Pilot Deployment (2-3 events)

**Pilot Event 1:**
- Deploy both Tauri app and Phoenix server
- Close monitoring and on-site support
- Gather feedback from photographer and viewers
- Document issues and pain points
- Measure performance metrics
- Test ingestion workflow under real conditions

**Post-Pilot 1:**
- Address critical issues
- Implement quick wins from feedback
- Refine documentation
- Tauri app bug fixes

**Pilot Event 2:**
- Deploy with improvements
- Less hands-on support (test independence)
- Validate fixes from Pilot 1
- Final feedback gathering

**Pilot Event 3 (Optional):**
- Confidence builder
- Final validation before full launch
- Performance verification at scale

**Deliverables:**
- Battle-tested application
- Updated documentation based on real-world usage
- Known issues and roadmap for v1.1

---

### Phase 5: Launch (Ongoing)

- Full release to production use
- Support for multiple events
- Iterate based on user feedback
- Plan Phase 2 features

**Total Timeline: 13-17 weeks from start to production-ready**

---

## Future Enhancements (Post-MVP)

### Phase 2 Features

- **QR code generation** - Print QR codes for direct competitor access
- **Photo favoriting/bookmarking** - Viewers can mark favorites
- **Download individual photos** - With watermarking option
- **Print kiosk integration** - On-site printing capability
- **Email galleries** - Send photo links to participants post-event
- **Batch operations** - Select multiple photos for operations
- **Photo ratings** - Flag best shots for photographer
- **Mobile browsing from stands** - Allow parents to browse on personal phones
  - Requires careful consideration of network capacity
  - May need WiFi infrastructure upgrade
  - QR code scanning for easy competitor access
  - Progressive Web App (PWA) for better mobile experience
  - This is an exciting future direction but needs infrastructure planning

### Phase 3 Features

- **E-commerce integration** - Photo purchasing and payment
- **Facial recognition** - Auto-tagging competitors in photos
- **Cloud backup** - Automatic backup to cloud storage post-event
- **Remote access** - Access photos from anywhere after event
- **Mobile apps** - Native iOS/Android apps
- **Photo editing** - Basic crop, rotate, enhance tools
- **Analytics dashboard** - Views, popular photos, user behavior
- **Social media integration** - Share directly to social platforms
- **Multi-event support** - Manage multiple events concurrently
- **Competitor portal** - Allow competitors to claim and manage their photos

### Phase 4 Features (Speculative)

- **Live streaming** - Display photos on venue screens in real-time
- **AI-powered features** - Auto-crop, quality scoring, pose detection
- **Advanced search** - By event type, color, composition
- **Slideshow mode** - Automated photo display for digital signage
- **API for integrations** - Connect with event management software
- **White-label solution** - Rebrand for different photographers
- **Marketplace** - Connect photographers with event organizers

---

### Explicitly Deferred (Not in MVP)

**Features intentionally excluded from initial release:**

- **Mobile app** - Web interface sufficient, native apps not needed yet
- **E-commerce/payment** - Photo ordering and purchasing deferred to post-MVP
- **Social media sharing** - Direct sharing to Facebook/Instagram not in MVP
- **Video support** - Photos only, no video processing
- **RAW file processing** - JPEG only (RAW converted externally if needed)
- **Facial recognition** - Manual categorization sufficient for MVP
- **Multi-language support** - English only initially
- **Offline viewing station capability** - Requires live connection to server
- **Photo editing tools** - No crop/rotate/enhance in MVP
- **Print kiosk integration** - Deferred to Phase 2
- **Email/SMS notifications** - No automated notifications in MVP
- **User accounts for participants** - Open browsing only, no login
- **Photo rating/commenting** - Not needed for MVP
- **Automated backups** - Manual backup process sufficient initially
- **Analytics/reporting** - Basic counts only, no detailed analytics
- **Custom branding per event** - Single branding theme for MVP

**Rationale for Deferrals:**
- Focus on core photo browsing workflow
- Reduce complexity and development time
- Get to first event deployment faster
- Validate core assumptions before adding features
- These can be added incrementally based on real user needs

---

## Appendix

### Implementation Notes

**Technology Choices - Rationale:**

1. **Tauri ingestion app over web-based upload:**
   - Web browsers cannot access USB/card readers directly
   - Preserves existing proven workflow
   - Native performance for file operations
   - Barcode scanner integration possible
   - VueJS frontend leverages existing expertise

2. **Phoenix/Elixir over alternatives:**
   - Product owner highly familiar with stack
   - Real-time features built-in (LiveView, PubSub)
   - BEAM VM excels at concurrent operations
   - Already validated via prototypes

3. **Web-based UI over native apps (for viewing):**
   - Zero installation on viewing stations
   - Works on any device with browser
   - Single codebase for all platforms
   - Faster iteration during development
   - Browser kiosk mode handles "lockdown" needs

4. **NAS file storage over database/cloud:**
   - Faster ingestion (direct file copy)
   - No bandwidth bottleneck through Phoenix
   - Simple backup/archival strategy
   - Browsable filesystem for orders
   - No internet required at venues

5. **Two-phase file organization:**
   - Preserves source for troubleshooting
   - Flexible correction workflow
   - Clean archival structure when finalized
   - Traceable via database audit trail

6. **Physical file moves over virtual:**
   - Filesystem matches database reality
   - Easy to browse for orders
   - No confusion about file locations
   - Simple mental model for admin

**Development Philosophy:**

- Start simple, add complexity only when needed
- Real data testing from day one (full meet photos available)
- Performance optimization based on actual bottlenecks, not assumptions
- Pragmatic monitoring (Oban Web sufficient for MVP)
- Focus on core workflow, defer nice-to-haves

**Risk Mitigation:**

- Prototype validation reduces technical risk
- Real test data available before first event
- Incremental feature delivery
- Manual fallbacks for critical operations
- Clear error messages for troubleshooting

---

### Glossary

**Admin Interface:** The management interface used by photographers and event organizers to manage photos and the system.

**Viewing Station:** A device (tablet, laptop, or kiosk) used by participants and families to browse photos.

**Hierarchy:** The organizational structure of the event (e.g., Gym → Session → Event → Flight → Competitor).

**Node:** An individual unit within the hierarchy (e.g., a specific gym, session, or competitor).

**Breadcrumb Navigation:** A navigation pattern showing the path from the top level to the current location.

**Thumbnail:** A small, compressed version of a photo used for quick browsing and preview.

**Ingestion:** The process of discovering, processing, and making photos available in the system.

**File Watcher:** A background process that monitors the file system for new photos.

**NAS:** Network Attached Storage - a dedicated file storage device on the local network.

**Phoenix LiveView:** A server-side rendering framework for real-time web applications.

**PubSub:** Publish-Subscribe messaging pattern for broadcasting updates to multiple viewers.

---

### Configuration Reference

```elixir
# config/runtime.exs
config :event_photos,
  # Storage paths
  photo_root: System.get_env("PHOTO_ROOT") || "/mnt/nas/photos",
  thumbnail_root: System.get_env("THUMBNAIL_ROOT") || "/mnt/nas/thumbnails",
  
  # Optional: Direct NAS serving
  nas_http_base: System.get_env("NAS_HTTP_BASE"), # e.g., "http://nas.local:8080"
  
  # File watching
  enable_file_watcher: System.get_env("ENABLE_FILE_WATCHER", "true") == "true",
  watch_interval_ms: String.to_integer(System.get_env("WATCH_INTERVAL_MS", "1000")),
  
  # Processing
  thumbnail_size: String.to_integer(System.get_env("THUMBNAIL_SIZE", "400")),
  thumbnail_quality: String.to_integer(System.get_env("THUMBNAIL_QUALITY", "85")),
  max_concurrent_processing: System.to_integer(System.get_env("MAX_CONCURRENT_PROCESSING", "4")),
  
  # Ingestion behavior
  auto_create_nodes: System.get_env("AUTO_CREATE_NODES", "true") == "true",
  skip_duplicates: System.get_env("SKIP_DUPLICATES", "true") == "true",
  max_file_size_mb: String.to_integer(System.get_env("MAX_FILE_SIZE_MB", "50")),
  
  # Admin auth
  admin_password: System.get_env("ADMIN_PASSWORD") || raise("ADMIN_PASSWORD required")
```

---

### Environment Setup

```bash
# .env.example
PHOTO_ROOT=/mnt/nas/photos
THUMBNAIL_ROOT=/mnt/nas/thumbnails
NAS_HTTP_BASE=http://nas.local:8080

DATABASE_URL=postgresql://postgres:postgres@localhost/event_photos_prod

ADMIN_PASSWORD=your_secure_password_here

ENABLE_FILE_WATCHER=true
AUTO_CREATE_NODES=true
THUMBNAIL_SIZE=400
```

---

### Deployment Checklist

**Pre-Event Setup:**
- [ ] Server hardware ready (laptop/dedicated server)
- [ ] NAS configured and accessible
- [ ] PostgreSQL installed and running
- [ ] Application deployed (Mix release)
- [ ] Network configured (WiFi AP or ethernet)
- [ ] Viewing stations connected and tested
- [ ] Event created in system
- [ ] Hierarchy defined
- [ ] Folder structure created on NAS (if strict structure)
- [ ] File watcher running (or manual scan ready)
- [ ] Admin password set
- [ ] Backup strategy in place

**During Event:**
- [ ] Server running and accessible
- [ ] File watcher active (check dashboard)
- [ ] Monitor ingestion pipeline for errors
- [ ] Check viewing stations periodically
- [ ] Verify new photos appearing for viewers
- [ ] Have manual scan as backup

**Post-Event:**
- [ ] Verify all photos ingested successfully
- [ ] Review error log, handle any issues
- [ ] Backup database and photos
- [ ] Archive event (if not needed active)
- [ ] Gather feedback from users
- [ ] Document any issues for improvement

---

## Document Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-11-22 | Product Team | Initial document creation with full technical architecture |
| 1.1 | 2025-11-22 | Product Team | Clarified open questions, added two-phase file organization, three-version image processing pipeline, roster import details |
| 1.2 | 2025-11-22 | Product Team | Finalized all requirements, added prototype validation notes, complete technical decisions summary, marked ready for development |
| 1.3 | 2025-11-22 | Product Team | Added comprehensive photo ingestion workflow, detailed Tauri desktop app specification, barcode scanner integration, memory card workflow documentation |
| 1.4 | 2025-11-22 | Product Team | **Added complete ordering system**, shopping cart integration, order management, USB/print fulfillment workflows, product catalog, payment tracking |

---

## Recent Decisions Summary (v1.4 - FINAL)

### Image Processing
- **Three versions**: Original (pristine), Watermarked Preview (~1280px), Thumbnail (~400px)
- Thumbnails generated from preview (not original) for efficiency
- Watermark design TBD

### File Organization
- **Two-phase approach**: Ingestion path preserves source, optional finalization for clean archival
- Ingestion path tracks photographer, order sequence, source folders
- Physical file moves preferred (keeps filesystem browsable)
- Database tracks both ingestion and current paths

### Authentication & Privacy
- Viewing stations completely open (no auth required)
- Admin uses basic auth/simple password
- All photos publicly viewable at venue
- Watermarking on preview images to protect photographer

### Scope Decisions
- JPEG only (RAW converted before copying)
- Single active event per deployment
- Data archival out of scope for MVP
- Hardware planning handled separately

### Roster Import
- CSV format supported with team info
- PDF rotation sheets may require manual extraction
- Format: `{COMPETITOR_ID} {FIRST_NAME} {LAST_INITIAL}`
- Pre-event import with full CRUD capabilities (add, edit, delete, rename)

### Scale & Performance
- **Per competitor**: ~100 photos (20-40 per event × 2-3 events)
- **Per meet**: 1,000+ competitors = **100,000+ photos**
- **Viewing stations**: 15 concurrent
- **Storage**: ~325 GB per event (originals + previews + thumbnails)
- **Performance requirements**:
  - Parallel thumbnail generation (4+ cores)
  - Database indexing critical
  - Pagination/virtual scrolling required
  - Selective PubSub broadcasting

### Workflow Decisions
- **Photo sorting**: By filename (capture sequence)
- **Finalization**: Manual trigger when verified correct
- **Roster timing**: Import before event, editable during
- **Error handling**: Retry mechanism with admin review

---

## Notes & Next Steps

### Key Technical Decisions Summary

All major architectural decisions have been finalized:

| Decision Area | Choice | Rationale |
|--------------|--------|-----------|
| **Backend Framework** | Phoenix/Elixir | Familiar stack, real-time built-in, validated via prototypes |
| **Database** | PostgreSQL | Robust, good for hierarchical queries, team experience |
| **Image Processing** | libvips (via vix) | Extremely fast, validated on Apple Silicon |
| **File Storage** | NAS with two-phase organization | Fast ingestion, browsable, traceable |
| **File Serving** | Hybrid: Phoenix + optional nginx | Start simple, optimize if needed |
| **Background Jobs** | Oban | Reliable, good monitoring, proven track record |
| **Frontend** | Phoenix LiveView + Tailwind | Minimal JS, real-time updates, rapid development |
| **UI Approach** | Web-based (not native) | Cross-platform, zero install, fast iteration |
| **Authentication** | Basic auth for admin, open for viewers | Appropriate for venue context |
| **Photo Formats** | JPEG only | Sufficient for events, manageable storage |

### Implementation Priorities

**Phase 1: Foundation (Week 1-2)**
1. Phoenix app structure and contexts
2. Database schema and migrations
3. File watcher implementation
4. Basic Oban job setup

**Phase 2: Admin Interface (Week 3-4)**
5. Event and hierarchy management
6. Photo ingestion monitoring
7. File browser and photo management
8. Roster import

**Phase 3: Viewer Interface (Week 5-6)**
9. Breadcrumb navigation
10. Search functionality
11. Photo gallery with pagination
12. Real-time updates

**Phase 4: Polish & Testing (Week 7-8)**
13. Test with full 100K photo dataset
14. Performance optimization
15. Error handling refinement
16. Documentation

### Ready for Development

✅ Requirements complete and documented  
✅ Architecture decisions finalized  
✅ Database schema designed  
✅ File workflows defined  
✅ Technical validation completed (prototypes)  
✅ Test data available (real meet photos)  
✅ Scale requirements understood (100K photos)  
✅ Technology stack selected and validated  

**Next Action:** Begin Phoenix application structure design

---

## Notes & Next Steps (Original)

**Immediate Next Steps:**
1. Review and approve this document
2. Answer open questions (section 11)
3. Create detailed UI mockups
4. Set up development environment
5. Begin Phase 1 development

**Questions to Resolve:**
- Viewer authentication approach (completely open?)
- Watermarking requirements
- Multi-event support in MVP
- File format support (JPEG only or include RAW?)
- Hardware recommendations detail level

**Parking Lot (Future Discussions):**
- E-commerce integration approach
- Print kiosk vendor selection
- Facial recognition feasibility and privacy
- Mobile app priority and timeline
- Cloud backup provider selection
