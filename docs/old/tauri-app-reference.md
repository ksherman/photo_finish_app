# Tauri Ingestion App - Quick Reference

## Purpose
Desktop application for ingesting photos from memory cards into the event photo management system.

## Why Desktop App?
Web browsers cannot access:
- USB drives/memory card readers
- Local file system directly
- Device enumeration (drive letters)

## Technology Stack
- **Backend:** Rust (Tauri framework)
- **Frontend:** VueJS 3 + Tailwind CSS
- **Platform:** macOS (Apple Silicon primary)

## Core Features

### 1. Session Configuration
- Set root destination path
- Load photographer profile
- Select card reader
- Load roster (from Phoenix API or file)

### 2. Memory Card Workflow
**Current Real-World Process:**
1. Photographer has 8 cards + 8 envelopes
2. Each rotation (e.g., "Group 1A/Beam") uses one card
3. Photographer creates folder per competitor on card
4. Card placed in envelope with notes
5. Staff collects envelopes, brings to station

**Tauri App Process:**
1. Insert memory card
2. Scan barcode on envelope → Auto-fills: "Group 1A/Beam"
3. Click "Copy Files" → Copies to NAS
4. Rename folders: "EOS100" → "1022 Kevin S"
5. Notify Phoenix → Processing begins

### 3. Key UI Screens

**Main Screen:**
```
┌────────────────────────────────────┐
│ Session: Gym A / Session 3         │
│ Photographer: KDS                  │
├────────────────────────────────────┤
│ Card Reader: /Volumes/EOS_DIGITAL  │
│ Status: ● Card Ready (1,234 files) │
│                                    │
│ Envelope Code:                     │
│ [Group 1A/Beam_________]  [Clear] │
│                                    │
│ Destination:                       │
│ /nas/.../Gym A/Session 3/          │
│ Group 1A/Beam/0001/               │
│                                    │
│ [Copy Files to Server]             │
│                                    │
│ Progress: ██████░░░░ 756/1234      │
└────────────────────────────────────┘
```

**Folder Renaming Screen:**
```
┌────────────────────────────────────┐
│ Rename Folders                     │
├────────────────────────────────────┤
│ EOS100 → [1022 Kevin S ▼]   12 photos │
│ EOS101 → [1023 Sarah J ▼]   15 photos │
│ EOS102 → [1024 Emma W  ▼]   18 photos │
│ EOS103 → [Skip (empty)]       0 photos │
├────────────────────────────────────┤
│ [Auto-Assign] [Apply All] [Cancel]│
└────────────────────────────────────┘
```

### 4. File Organization
Files copied to structure:
```
/nas/event-name/KDS/Gym A/Session 3/Group 1A/Beam/0001/EOS100/IMG_001.jpg
                 │   │     │         │           │    │     │
                 │   │     │         │           │    │     └─ Original folder
                 │   │     │         │           │    └─ Order number
                 │   │     │         │           └─ Envelope code
                 │   │     │         └─ Session
                 │   │     └─ Gym
                 │   └─ Photographer
                 └─ Event
```

### 5. Phoenix API Integration

**Endpoints:**
```
GET  /api/roster/:event_id
  → Get competitor roster

POST /api/ingestion/notify
  → Notify of completed copy
  Body: {
    event_id,
    photographer,
    envelope_code,
    order_number,
    destination_path,
    file_count
  }

POST /api/ingestion/rename
  → Report folder renames
  Body: {
    renames: [{original, new, photo_count}]
  }
```

### 6. Error Handling
- Card ejected during copy
- Insufficient disk space
- Network/Phoenix unavailable
- Corrupt files
- Read/write errors

## Development Phases

### Phase 1: MVP (1-2 weeks)
- Basic UI and card reader detection
- Manual envelope code entry (no barcode)
- File copy with progress
- Simple folder renaming
- Phoenix API notification

### Phase 2: Enhanced (1 week)
- Barcode scanner integration
- Roster-based auto-suggest
- Bulk rename operations
- Offline mode
- Error handling

### Phase 3: Polish (1 week)
- Keyboard shortcuts
- History logging
- Performance optimization
- Testing with real data

## Key Benefits

1. **Preserves Workflow**
   - Maintains proven process
   - Checkpoints at each stage
   - Order sequence preserved

2. **Native Performance**
   - Fast file operations
   - Direct USB access
   - Responsive UI

3. **VueJS Frontend**
   - Leverage existing expertise
   - Rapid UI development
   - Familiar tooling

4. **Barcode Integration**
   - Scanner acts as keyboard
   - Auto-populates fields
   - Reduces typing errors

## Integration with Phoenix

1. **Tauri app copies files** → NAS
2. **Tauri app notifies Phoenix** via API
3. **Phoenix file watcher** detects new files
4. **Processing pipeline** starts (Oban jobs)
5. **Photos appear** in viewer interface

## Alternative Considered

- **Web-based upload:** Too slow, no USB access
- **CLI commands:** Not user-friendly
- **Shell scripts:** Too basic, no UI
- **Electron app:** Heavier than Tauri

**Verdict:** Tauri is the right choice

## Open Questions

1. Barcode scanner model/compatibility?
2. Multiple card readers simultaneously?
3. Windows support needed?
4. Offline mode critical or nice-to-have?

## Success Criteria

- Copy 1,000 photos in < 5 minutes
- Zero data loss
- Easy to learn (< 15 min training)
- Reliable barcode scanning
- Clear error messages
- Works offline (optional Phoenix sync)
