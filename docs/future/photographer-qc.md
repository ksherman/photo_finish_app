# Photographer QC Review Screen — Future Feature Idea

## The Problem

At a large meet with multiple photographers, it's hard to ensure consistent quality across the board:
- Color temperature and exposure can vary between shooters
- Some photographers may miss key moments or capture better angles than others
- Currently there's no tooling to compare output side-by-side

## The Idea

Build a **Photographer Review screen** in the admin area that lets the lead photographer or admin compare photo output across photographers in real time (or post-session).

Since the ingest app already tracks which device/reader photos came from, we have the data to attribute photos to photographers.

## What It Could Show

- Side-by-side grid views, one column (or panel) per photographer
- Filters: by event session, apparatus, time window
- Quick visual scan for: exposure consistency, color balance, moment quality
- Possibly a "flag" mechanism to surface outliers or photos needing correction

## Key Enablers Already In Place

- Ingest app tracks source (card reader / device) → photo attribution is feasible
- Photos are stored with ingestion metadata
- Admin LiveView patterns already exist

## Open Questions for Later

- Do we track photographer-per-card-reader, or photographer-per-session assignment?
- Is this a live view (updates as photos come in) or a post-session review tool?
- What actions can be taken from this screen? (flag, reassign, delete?)
- Color/exposure comparison: purely visual, or do we want computed metrics (histogram, etc.)?

## Priority

Low — capture for future sprint planning. No code yet.
