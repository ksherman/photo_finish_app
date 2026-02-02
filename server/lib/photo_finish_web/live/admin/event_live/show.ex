defmodule PhotoFinishWeb.Admin.EventLive.Show do
  use PhotoFinishWeb, :live_view

  alias PhotoFinish.Events.FolderGenerator
  alias PhotoFinish.Photos.Photo
  alias PhotoFinish.Photos.LocationBrowser

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.admin flash={@flash}>
      <div class="min-h-screen bg-gray-50">
        <%!-- Header --%>
        <div class="bg-white border-b border-gray-200 px-6 py-4">
          <div class="max-w-4xl mx-auto">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-4">
                <.button_link
                  navigate={~p"/admin/events"}
                  variant="outline"
                  color="natural"
                  size="small"
                >
                  <.icon name="hero-arrow-left" class="w-4 h-4" />
                </.button_link>
                <div>
                  <h1 class="text-xl font-bold text-gray-900">{@event.name}</h1>
                  <p class="text-sm text-gray-500">{@event.slug}</p>
                </div>
                <.badge color={if @event.status == :active, do: "success", else: "neutral"}>
                  {Phoenix.Naming.humanize(@event.status)}
                </.badge>
              </div>
              <.button_link
                navigate={~p"/admin/events/#{@event}/edit"}
                size="small"
                variant="outline"
                color="natural"
              >
                <.icon name="hero-pencil" class="w-4 h-4 mr-1" /> Edit Event
              </.button_link>
            </div>
          </div>
        </div>

        <%!-- Main Content --%>
        <div class="max-w-4xl mx-auto py-8 px-6 space-y-6">
          <%!-- Configuration Card --%>
          <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
            <div class="px-6 py-4 bg-gray-50 border-b border-gray-200">
              <h2 class="text-lg font-semibold text-gray-900">Configuration</h2>
            </div>
            <div class="p-6">
              <dl class="grid grid-cols-1 sm:grid-cols-2 gap-x-8 gap-y-4">
                <div>
                  <dt class="text-sm font-medium text-gray-500">Storage Root</dt>
                  <dd class="mt-1 text-sm text-gray-900 font-mono break-all">
                    {@event.storage_root || "Not configured"}
                  </dd>
                </div>
                <div>
                  <dt class="text-sm font-medium text-gray-500">Order Code</dt>
                  <dd class="mt-1 text-sm text-gray-900 font-mono">
                    {@event.order_code || "-"}
                  </dd>
                </div>
                <div>
                  <dt class="text-sm font-medium text-gray-500">Gyms</dt>
                  <dd class="mt-1 text-sm text-gray-900">
                    {@event.num_gyms} ({@gym_letters})
                  </dd>
                </div>
                <div>
                  <dt class="text-sm font-medium text-gray-500">Sessions</dt>
                  <dd class="mt-1 text-sm text-gray-900">
                    {@event.sessions_per_gym} per gym ({@total_sessions} total)
                  </dd>
                </div>
                <%= if @event.starts_at do %>
                  <div>
                    <dt class="text-sm font-medium text-gray-500">Start Date</dt>
                    <dd class="mt-1 text-sm text-gray-900">
                      {Calendar.strftime(@event.starts_at, "%B %d, %Y at %I:%M %p")}
                    </dd>
                  </div>
                <% end %>
                <%= if @event.ends_at do %>
                  <div>
                    <dt class="text-sm font-medium text-gray-500">End Date</dt>
                    <dd class="mt-1 text-sm text-gray-900">
                      {Calendar.strftime(@event.ends_at, "%B %d, %Y at %I:%M %p")}
                    </dd>
                  </div>
                <% end %>
                <%= if @event.tax_rate_basis_points do %>
                  <div>
                    <dt class="text-sm font-medium text-gray-500">Tax Rate</dt>
                    <dd class="mt-1 text-sm text-gray-900">
                      {Float.round(@event.tax_rate_basis_points / 100, 2)}%
                    </dd>
                  </div>
                <% end %>
              </dl>

              <%= if @event.description do %>
                <div class="mt-6 pt-4 border-t border-gray-100">
                  <dt class="text-sm font-medium text-gray-500">Description</dt>
                  <dd class="mt-1 text-sm text-gray-700">{@event.description}</dd>
                </div>
              <% end %>
            </div>
          </div>

          <%!-- Photos Card --%>
          <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
            <div class="px-6 py-4 bg-gray-50 border-b border-gray-200">
              <div class="flex items-center justify-between">
                <h2 class="text-lg font-semibold text-gray-900">Photos</h2>
                <div class="flex items-center gap-4">
                  <div class="flex gap-2 text-sm">
                    <span class="text-green-600">{@photo_counts.ready} ready</span>
                    <span class="text-gray-300">|</span>
                    <span class="text-blue-600">{@photo_counts.pending} pending</span>
                    <span class="text-gray-300">|</span>
                    <span class="text-red-600">{@photo_counts.error} errors</span>
                  </div>
                  <.button
                    phx-click="scan_now"
                    disabled={is_nil(@event.storage_root) || @scanning}
                    size="small"
                    variant="outline"
                    color="natural"
                  >
                    <%= if @scanning do %>
                      <svg class="animate-spin w-4 h-4 mr-1" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                      </svg>
                      Scanning...
                    <% else %>
                      <.icon name="hero-magnifying-glass" class="w-4 h-4 mr-1" /> Scan
                    <% end %>
                  </.button>
                </div>
              </div>

              <%!-- Progress bar (shows when processing) --%>
              <%= if @photo_counts.pending > 0 do %>
                <div class="mt-3">
                  <div class="flex items-center justify-between text-xs text-gray-600 mb-1">
                    <span class="flex items-center gap-1">
                      <span class="inline-block w-2 h-2 bg-blue-500 rounded-full animate-pulse"></span>
                      Processing photos...
                    </span>
                    <span>{@photo_counts.ready + @photo_counts.error} of {@photo_counts.total} complete</span>
                  </div>
                  <div class="w-full bg-gray-200 rounded-full h-2 overflow-hidden">
                    <div
                      class="h-2 rounded-full bg-gradient-to-r from-blue-500 to-green-500 transition-all duration-300"
                      style={"width: #{@photo_counts.progress_percent}%"}
                    >
                    </div>
                  </div>
                </div>
              <% end %>
            </div>
            <div class="p-6">
              <%= if is_nil(@event.storage_root) do %>
                <p class="text-sm text-amber-600">
                  Set a storage root in the event settings to enable photo browsing.
                </p>
              <% else %>
                <%!-- Breadcrumb --%>
                <nav class="mb-4">
                  <ol class="flex items-center gap-2 text-sm">
                    <li>
                      <button
                        phx-click="browse_to"
                        phx-value-index="0"
                        class={[
                          "hover:text-blue-600",
                          @browser_path == [] && "font-semibold text-gray-900",
                          @browser_path != [] && "text-blue-600"
                        ]}
                      >
                        All Photos
                      </button>
                    </li>
                    <%= for {segment, index} <- Enum.with_index(@browser_path) do %>
                      <li class="text-gray-400">/</li>
                      <li>
                        <button
                          phx-click="browse_to"
                          phx-value-index={index + 1}
                          class={[
                            "hover:text-blue-600",
                            index == length(@browser_path) - 1 && "font-semibold text-gray-900",
                            index != length(@browser_path) - 1 && "text-blue-600"
                          ]}
                        >
                          {LocationBrowser.format_value_at_index(segment, index)}
                        </button>
                      </li>
                    <% end %>
                  </ol>
                </nav>

                <%!-- Level indicator --%>
                <%= if !LocationBrowser.at_leaf_level?(@browser_path) do %>
                  <p class="text-xs text-gray-500 mb-4">
                    Select a {LocationBrowser.level_label(LocationBrowser.current_level(@browser_path))}
                  </p>
                <% end %>

                <%!-- Children cards or Photo grid --%>
                <%= if @browser_children != [] do %>
                  <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-3">
                    <%= for child <- @browser_children do %>
                      <% current_level = LocationBrowser.current_level(@browser_path) %>
                      <button
                        phx-click="browse_into"
                        phx-value-name={child.name}
                        class="p-4 bg-gray-50 rounded-lg border border-gray-200 hover:border-blue-300 hover:bg-blue-50 transition-colors text-left"
                      >
                        <p class="font-medium text-gray-900 truncate">
                          {LocationBrowser.format_value(current_level, child.name)}
                        </p>
                        <p class="text-sm text-gray-500">{child.count} photos</p>
                      </button>
                    <% end %>
                  </div>
                <% end %>

                <%= if @browser_folders != [] do %>
                  <%!-- Expand/Collapse all buttons --%>
                  <div class="flex justify-end gap-2 mb-3">
                    <button
                      phx-click="expand_all_folders"
                      class="text-xs text-blue-600 hover:text-blue-800"
                    >
                      Expand all
                    </button>
                    <span class="text-gray-300">|</span>
                    <button
                      phx-click="collapse_all_folders"
                      class="text-xs text-blue-600 hover:text-blue-800"
                    >
                      Collapse all
                    </button>
                  </div>

                  <%!-- Accordion folders --%>
                  <div class="space-y-2">
                    <%= for folder <- @browser_folders do %>
                      <div class="border border-gray-200 rounded-lg overflow-hidden">
                        <button
                          phx-click="toggle_folder"
                          phx-value-folder={folder.folder}
                          class="w-full flex items-center justify-between p-3 bg-gray-50 hover:bg-gray-100 transition-colors"
                        >
                          <div class="flex items-center gap-3">
                            <.icon
                              name={if MapSet.member?(@expanded_folders, folder.folder), do: "hero-chevron-down", else: "hero-chevron-right"}
                              class="w-4 h-4 text-gray-500"
                            />
                            <span class="font-medium text-gray-900">{folder.folder}</span>
                          </div>
                          <span class="text-sm text-gray-500">{folder.count} photos</span>
                        </button>

                        <%= if MapSet.member?(@expanded_folders, folder.folder) do %>
                          <div class="p-3 bg-white">
                            <div class="grid grid-cols-4 sm:grid-cols-6 md:grid-cols-8 gap-2">
                              <%= for photo <- folder.photos do %>
                                <div class="relative group">
                                  <img
                                    src={photo_thumbnail_url(photo)}
                                    alt={photo.filename}
                                    class="w-full aspect-square object-cover rounded bg-gray-100"
                                    loading="lazy"
                                  />
                                  <div class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/60 to-transparent p-1 rounded-b">
                                    <p class="text-[10px] text-white truncate">{photo.filename}</p>
                                  </div>
                                  <%= if photo.status == :error do %>
                                    <div class="absolute top-1 right-1">
                                      <span class="inline-flex items-center justify-center w-4 h-4 bg-red-500 rounded-full">
                                        <.icon name="hero-exclamation-triangle" class="w-2 h-2 text-white" />
                                      </span>
                                    </div>
                                  <% end %>
                                </div>
                              <% end %>
                            </div>
                          </div>
                        <% end %>
                      </div>
                    <% end %>
                  </div>
                <% end %>

                <%= if @browser_children == [] && @browser_folders == [] do %>
                  <p class="text-sm text-gray-500 text-center py-8">
                    No photos found at this location.
                  </p>
                <% end %>
              <% end %>
            </div>
          </div>

          <%!-- Quick Actions Card --%>
          <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
            <div class="px-6 py-4 bg-gray-50 border-b border-gray-200">
              <h2 class="text-lg font-semibold text-gray-900">Quick Actions</h2>
            </div>
            <div class="p-6">
              <div class="flex flex-wrap gap-3">
                <.button_link
                  navigate={~p"/admin/events/#{@event.id}/import-roster"}
                  size="small"
                  variant="outline"
                  color="primary"
                >
                  <.icon name="hero-arrow-up-tray" class="w-4 h-4 mr-1" /> Import Roster
                </.button_link>
                <.button_link
                  navigate={~p"/admin/events/#{@event.id}/folders"}
                  size="small"
                  variant="outline"
                  color="primary"
                >
                  <.icon name="hero-folder" class="w-4 h-4 mr-1" /> Associate Folders
                </.button_link>
                <.button_link
                  navigate={~p"/admin/events/#{@event}/edit"}
                  size="small"
                  variant="outline"
                  color="natural"
                >
                  <.icon name="hero-pencil" class="w-4 h-4 mr-1" /> Edit Event
                </.button_link>
                <.button_link
                  navigate={~p"/admin/events"}
                  size="small"
                  variant="outline"
                  color="natural"
                >
                  <.icon name="hero-list-bullet" class="w-4 h-4 mr-1" /> All Events
                </.button_link>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Layouts.admin>
    """
  end

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    event = Ash.get!(PhotoFinish.Events.Event, id)

    # Subscribe to real-time photo processing updates
    if connected?(socket) do
      Phoenix.PubSub.subscribe(PhotoFinish.PubSub, "photos:event:#{id}")
    end

    gym_letters = format_gym_letters(event.num_gyms)
    total_sessions = event.num_gyms * event.sessions_per_gym

    {:ok,
     socket
     |> assign(:page_title, event.name)
     |> assign(:event, event)
     |> assign(:gym_letters, gym_letters)
     |> assign(:total_sessions, total_sessions)
     |> assign(:photo_counts, load_photo_counts(id))
     |> assign(:browser_path, [])
     |> assign(:expanded_folders, MapSet.new())
     |> assign(:scanning, false)
     |> load_browser_data(id, [])}
  end

  defp load_browser_data(socket, event_id, path) do
    if LocationBrowser.at_leaf_level?(path) do
      folders = LocationBrowser.get_photo_folders(event_id, path)

      socket
      |> assign(:browser_children, [])
      |> assign(:browser_folders, folders)
      |> assign(:expanded_folders, MapSet.new())
    else
      children = LocationBrowser.get_children(event_id, path)

      socket
      |> assign(:browser_children, children)
      |> assign(:browser_folders, [])
      |> assign(:expanded_folders, MapSet.new())
    end
  end

  defp photo_thumbnail_url(photo) do
    # For now, serve via a controller route
    # Falls back to a placeholder if no thumbnail exists
    if photo.thumbnail_path && File.exists?(photo.thumbnail_path) do
      "/admin/photos/thumbnail/#{photo.id}"
    else
      "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'%3E%3Crect fill='%23e5e7eb' width='100' height='100'/%3E%3Ctext x='50' y='55' text-anchor='middle' fill='%239ca3af' font-size='12'%3ENo thumb%3C/text%3E%3C/svg%3E"
    end
  end

  defp format_gym_letters(num_gyms) when num_gyms >= 1 do
    letters =
      1..num_gyms
      |> Enum.map(&FolderGenerator.gym_letter/1)
      |> Enum.join(", ")

    letters
  end

  defp load_photo_counts(event_id) do
    photos =
      Ash.read!(Photo)
      |> Enum.filter(&(&1.event_id == event_id))

    total = length(photos)
    ready = Enum.count(photos, &(&1.status == :ready))
    discovered = Enum.count(photos, &(&1.status == :discovered))
    processing = Enum.count(photos, &(&1.status == :processing))
    error = Enum.count(photos, &(&1.status == :error))
    pending = discovered + processing

    %{
      total: total,
      ready: ready,
      discovered: discovered,
      processing: processing,
      pending: pending,
      error: error,
      progress_percent: if(total > 0, do: round((ready + error) / total * 100), else: 0)
    }
  end

  @impl true
  def handle_event("scan_now", _params, socket) do
    # Show scanning state immediately
    send(self(), :do_scan)
    {:noreply, assign(socket, :scanning, true)}
  end

  def handle_event("browse_into", %{"name" => name}, socket) do
    new_path = socket.assigns.browser_path ++ [name]

    socket =
      socket
      |> assign(:browser_path, new_path)
      |> load_browser_data(socket.assigns.event.id, new_path)

    {:noreply, socket}
  end

  def handle_event("browse_to", %{"index" => index}, socket) do
    index = String.to_integer(index)
    new_path = Enum.take(socket.assigns.browser_path, index)

    socket =
      socket
      |> assign(:browser_path, new_path)
      |> load_browser_data(socket.assigns.event.id, new_path)

    {:noreply, socket}
  end

  def handle_event("toggle_folder", %{"folder" => folder}, socket) do
    expanded = socket.assigns.expanded_folders

    new_expanded =
      if MapSet.member?(expanded, folder) do
        MapSet.delete(expanded, folder)
      else
        MapSet.put(expanded, folder)
      end

    {:noreply, assign(socket, :expanded_folders, new_expanded)}
  end

  def handle_event("expand_all_folders", _params, socket) do
    all_folders =
      socket.assigns.browser_folders
      |> Enum.map(& &1.folder)
      |> MapSet.new()

    {:noreply, assign(socket, :expanded_folders, all_folders)}
  end

  def handle_event("collapse_all_folders", _params, socket) do
    {:noreply, assign(socket, :expanded_folders, MapSet.new())}
  end

  @impl true
  def handle_info(:do_scan, socket) do
    socket =
      case PhotoFinish.Ingestion.scan_event(socket.assigns.event.id) do
        {:ok, result} ->
          socket
          |> put_flash(
            :info,
            "Scan complete. Found #{result.photos_new} new photos, #{result.photos_skipped} skipped."
          )
          |> assign(:photo_counts, load_photo_counts(socket.assigns.event.id))
          |> load_browser_data(socket.assigns.event.id, socket.assigns.browser_path)

        {:error, :directory_not_found} ->
          put_flash(socket, :error, "Storage directory not found.")

        {:error, reason} ->
          put_flash(socket, :error, "Scan failed: #{inspect(reason)}")
      end

    {:noreply, assign(socket, :scanning, false)}
  end

  def handle_info({:photo_status_changed, _payload}, socket) do
    # Refresh counts and browser data when photos are processed
    socket =
      socket
      |> assign(:photo_counts, load_photo_counts(socket.assigns.event.id))
      |> load_browser_data(socket.assigns.event.id, socket.assigns.browser_path)

    {:noreply, socket}
  end
end
