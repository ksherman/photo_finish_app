# PhotoFinish - Viewer Interface

**Version:** 1.0  
**Date:** November 29, 2025

---

## Overview

Public-facing web interface for browsing photos at viewing stations. No authentication required. Optimized for tablets (10"+).

---

## Navigation Model

Breadcrumb card-based navigation through hierarchy:

```
Event → Gym → Session → Apparatus → Flight → Competitor → Photos
```

Each level displays cards for children. Final level shows photo grid.

### Example Flow

1. **Event root:** Cards for "Gym A", "Gym B", "Gym C"
2. **Click "Gym A":** Cards for "Session 1", "Session 2", "Session 3"
3. **Click "Session 2":** Cards for "Floor", "Beam", "Bars", "Vault"
4. **Click "Floor":** Cards for "A Flight", "B Flight"
5. **Click "A Flight":** Cards for competitors with photo counts
6. **Click "1022 Kevin S":** Photo grid

---

## Page Layouts

### Hierarchy Browser

```
┌─────────────────────────────────────────────────────┐
│ [🔍 Search]                          PhotoFinish    │
├─────────────────────────────────────────────────────┤
│ Event > Gym A > Session 2 > Floor                   │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐             │
│  │ A Flight│  │ B Flight│  │         │             │
│  │  124 📷 │  │  118 📷 │  │         │             │
│  └─────────┘  └─────────┘  └─────────┘             │
│                                                     │
└─────────────────────────────────────────────────────┘
```

### Competitor List (Final Hierarchy Level)

```
┌─────────────────────────────────────────────────────┐
│ [🔍 Search]                          PhotoFinish    │
├─────────────────────────────────────────────────────┤
│ Event > Gym A > Session 2 > Floor > A Flight        │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌────────┐ │
│  │ 1022    │  │ 1023    │  │ 1024    │  │ 1025   │ │
│  │ Kevin S │  │ Sarah J │  │ Emma W  │  │ Lily M │ │
│  │  24 📷  │  │  31 📷  │  │  28 📷  │  │  19 📷 │ │
│  └─────────┘  └─────────┘  └─────────┘  └────────┘ │
│                                                     │
│  ┌─────────┐  ┌─────────┐                          │
│  │ 1026    │  │ 1027    │                          │
│  │ Mia T   │  │ Ava R   │                          │
│  │  22 📷  │  │  27 📷  │                          │
│  └─────────┘  └─────────┘                          │
│                                                     │
└─────────────────────────────────────────────────────┘
```

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
  
  from(c in Competitor,
    where: c.event_id == ^event_id,
    where: c.is_active == true,
    where: ilike(c.first_name, ^pattern)
        or ilike(c.last_name, ^pattern)
        or ilike(c.competitor_number, ^pattern)
        or ilike(c.team_name, ^pattern),
    left_join: p in Photo, on: p.competitor_id == c.id and p.status == "ready",
    group_by: c.id,
    select: %{
      id: c.id,
      display_name: c.display_name,
      competitor_number: c.competitor_number,
      team_name: c.team_name,
      photo_count: count(p.id)
    },
    order_by: [c.last_name, c.first_name],
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
    # Subscribe to current node
    Phoenix.PubSub.subscribe(PhotoFinish.PubSub, "photos:node:#{node_id}")
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

def load_photos(competitor_id, page \\ 1) do
  offset = (page - 1) * @photos_per_page
  
  from(p in Photo,
    where: p.competitor_id == ^competitor_id,
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

### HierarchyCard

```elixir
defmodule PhotoFinishWeb.Components.HierarchyCard do
  use Phoenix.Component
  
  def card(assigns) do
    ~H"""
    <a href={@href} class="block p-4 bg-white rounded-lg shadow hover:shadow-md">
      <h3 class="font-semibold text-lg"><%= @name %></h3>
      <p class="text-gray-500 text-sm"><%= @photo_count %> photos</p>
    </a>
    """
  end
end
```

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
/                           # Event root (hierarchy browser)
/browse/:node_slug          # Any hierarchy level
/competitor/:competitor_id  # Competitor photo grid
/photo/:photo_id            # Direct link to photo (opens lightbox)
/search?q=kevin             # Search results
/cart                       # Shopping cart
/checkout                   # Checkout form
/order/:order_number        # Order confirmation
```

---

## Error States

- **No photos found:** "No photos yet. Check back soon!"
- **Loading:** Skeleton placeholders
- **Network error:** "Connection lost. Retrying..."
- **Search no results:** "No competitors found for '{query}'"
