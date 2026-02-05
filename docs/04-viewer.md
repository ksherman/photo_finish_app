# PhotoFinish - Viewer Interface

**Version:** 1.0  
**Date:** November 29, 2025

---

## Overview

Public-facing web interface for browsing photos at viewing stations. No authentication required. Optimized for tablets (10"+).

---

## Navigation Model

Search-first navigation with optional browsing by location:

```
Home (Search + Featured Photos) → Competitor → Photos
```

Competitors are found via search (name, number, team). Photos are organized by flat location fields (gym, session, group, apparatus) parsed from folder structure.

---

## Page Layouts

### Photo Grid

```
┌─────────────────────────────────────────────────────┐
│ [🔍]  [🛒 Cart (3)]                  PhotoFinish    │
├─────────────────────────────────────────────────────┤
│ ← Back   1022 Kevin S - Floor (24 photos)           │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐        │
│  │ 📷 │ │ 📷 │ │ 📷 │ │ 📷 │ │ 📷 │ │ 📷 │        │
│  │[+] │ │[+] │ │[✓] │ │[+] │ │[+] │ │[✓] │        │
│  └────┘ └────┘ └────┘ └────┘ └────┘ └────┘        │
│                                                     │
│  ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐        │
│  │ 📷 │ │ 📷 │ │ 📷 │ │ 📷 │ │ 📷 │ │ 📷 │        │
│  │[+] │ │[+] │ │[+] │ │[+] │ │[+] │ │[+] │        │
│  └────┘ └────┘ └────┘ └────┘ └────┘ └────┘        │
│                                                     │
│  Page 1 of 2   [1] [2] [→]                         │
└─────────────────────────────────────────────────────┘
```

### Lightbox (Full Photo View)

```
┌─────────────────────────────────────────────────────┐
│                                        [×] Close    │
│                                                     │
│    [←]         ┌───────────────────┐         [→]   │
│                │                   │               │
│                │                   │               │
│                │    Full Preview   │               │
│                │     (1280px)      │               │
│                │                   │               │
│                │                   │               │
│                └───────────────────┘               │
│                                                     │
│                    3 of 24                         │
│                 [🛒 Add to Cart]                    │
└─────────────────────────────────────────────────────┘
```

---

## Search

### Behavior

- Search bar always visible in header
- Searches competitors by: name, number, team
- Auto-complete with top 10 matches
- Click result → navigate directly to competitor's photos

### Implementation

```elixir
def search_competitors(event_id, query) do
  pattern = "%#{query}%"

  from(ec in EventCompetitor,
    where: ec.event_id == ^event_id,
    where: ec.is_active == true,
    where: ilike(ec.display_name, ^pattern)
        or ilike(ec.competitor_number, ^pattern)
        or ilike(ec.team_name, ^pattern),
    left_join: p in Photo, on: p.event_competitor_id == ec.id and p.status == "ready",
    group_by: ec.id,
    select: %{
      id: ec.id,
      display_name: ec.display_name,
      competitor_number: ec.competitor_number,
      team_name: ec.team_name,
      photo_count: count(p.id)
    },
    order_by: [ec.display_name],
    limit: 10
  )
  |> Repo.all()
end
```

---

## Real-Time Updates

### LiveView Subscriptions

```elixir
def mount(_params, _session, socket) do
  if connected?(socket) do
    # Subscribe to competitor's photos
    Phoenix.PubSub.subscribe(PhotoFinish.PubSub, "photos:competitor:#{competitor_id}")
  end
  {:ok, socket}
end

def handle_info({:new_photo, photo}, socket) do
  socket = update(socket, :photos, fn photos -> [photo | photos] end)
  socket = update(socket, :photo_count, &(&1 + 1))
  {:noreply, socket}
end
```

### New Photo Indicator

When new photos arrive:
- Badge shows "+N new photos"
- Photos prepend to grid (or append based on sort)
- Optional: toast notification

---

## Pagination

For competitors with many photos (100+):

```elixir
@photos_per_page 24

def load_photos(event_competitor_id, page \\ 1) do
  offset = (page - 1) * @photos_per_page

  from(p in Photo,
    where: p.event_competitor_id == ^event_competitor_id,
    where: p.status == "ready",
    order_by: [p.filename],
    limit: @photos_per_page,
    offset: ^offset
  )
  |> Repo.all()
end
```

Consider virtual scrolling for smoother UX on large galleries.

---

## Performance Optimizations

### Thumbnail Loading

- Lazy load images as they scroll into view
- Use `loading="lazy"` attribute
- Preload next page on hover/approach

### Image Sizing

```html
<img 
  src={@photo.thumbnail_url}
  width="320"
  height="213"
  loading="lazy"
  class="object-cover w-full aspect-[3/2]"
/>
```

### Caching

- Thumbnails should cache aggressively (long `Cache-Control`)
- Use fingerprinted URLs or ETags

---

## Touch Optimization

- Large tap targets (min 44px)
- Swipe gestures in lightbox (left/right)
- Pinch-to-zoom on full photos
- No hover-only interactions

---

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `←` / `→` | Previous/next photo in lightbox |
| `Escape` | Close lightbox |
| `/` | Focus search |
| `Backspace` | Go back one level |

---

## LiveView Components

### PhotoGrid

```elixir
def photo_grid(assigns) do
  ~H"""
  <div class="grid grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-2">
    <%= for photo <- @photos do %>
      <.photo_card photo={photo} in_cart={photo.id in @cart_photo_ids} />
    <% end %>
  </div>
  """
end
```

### PhotoCard

```elixir
def photo_card(assigns) do
  ~H"""
  <div class="relative group">
    <img 
      src={@photo.thumbnail_url} 
      phx-click="view_photo" 
      phx-value-id={@photo.id}
      class="w-full aspect-[3/2] object-cover rounded cursor-pointer"
      loading="lazy"
    />
    <button 
      phx-click="toggle_cart"
      phx-value-id={@photo.id}
      class={[
        "absolute bottom-2 right-2 p-2 rounded-full",
        @in_cart && "bg-green-500 text-white",
        !@in_cart && "bg-white/80 text-gray-700"
      ]}
    >
      <%= if @in_cart, do: "✓", else: "+" %>
    </button>
  </div>
  """
end
```

---

## URL Structure

```
/viewer                              # Home (search + featured photos)
/viewer/competitor/:competitor_id    # Competitor photo grid
/viewer/photo/:photo_id              # Direct link to photo (opens lightbox)
/viewer/search?q=kevin               # Search results
/viewer/cart                          # Shopping cart
/viewer/checkout                      # Checkout form
/viewer/order/:order_number           # Order confirmation
```

---

## Error States

- **No photos found:** "No photos yet. Check back soon!"
- **Loading:** Skeleton placeholders
- **Network error:** "Connection lost. Retrying..."
- **Search no results:** "No competitors found for '{query}'"
