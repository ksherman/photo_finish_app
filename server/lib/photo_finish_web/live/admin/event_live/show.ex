defmodule PhotoFinishWeb.Admin.EventLive.Show do
  use PhotoFinishWeb, :live_view

  alias PhotoFinish.Photos.Photo

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.admin flash={@flash}>
      <div class="h-screen flex flex-col bg-gray-50">
        <%!-- Top Toolbar --%>
        <div class="bg-white border-b border-gray-200 px-6 py-3 flex-shrink-0">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-4">
              <.button_link navigate={~p"/admin/events"} variant="outline" color="natural" size="small">
                <.icon name="hero-arrow-left" class="w-4 h-4" />
              </.button_link>
              <div>
                <h1 class="text-xl font-bold text-gray-900">{@event.name}</h1>
                <p class="text-xs text-gray-500">{@event.slug}</p>
              </div>
              <.badge color={if @event.status == :active, do: "success", else: "neutral"}>
                {Phoenix.Naming.humanize(@event.status)}
              </.badge>
            </div>
            <div class="flex items-center gap-2">
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

        <%!-- Main Content Area --%>
        <div class="flex-1 flex overflow-hidden">
          <%!-- Left Sidebar - Tree Navigation --%>
          <div class="w-64 bg-white border-r border-gray-200 overflow-y-auto flex-shrink-0">
            <div class="p-4">
              <h3 class="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-3">
                Navigation
              </h3>
              <nav class="space-y-1">
                <button
                  phx-click="select_view"
                  phx-value-view="overview"
                  class={[
                    "w-full flex items-center px-3 py-2 text-sm rounded-lg transition-colors",
                    @current_view == "overview" &&
                      "bg-blue-50 text-blue-700 font-medium",
                    @current_view != "overview" &&
                      "text-gray-700 hover:bg-gray-50"
                  ]}
                >
                  <.icon name="hero-information-circle" class="w-4 h-4 mr-2" /> Overview
                </button>
                <button
                  phx-click="select_view"
                  phx-value-view="competitors"
                  class={[
                    "w-full flex items-center px-3 py-2 text-sm rounded-lg transition-colors",
                    @current_view == "competitors" &&
                      "bg-blue-50 text-blue-700 font-medium",
                    @current_view != "competitors" &&
                      "text-gray-700 hover:bg-gray-50"
                  ]}
                >
                  <.icon name="hero-users" class="w-4 h-4 mr-2" /> Competitors
                </button>
              </nav>
            </div>
          </div>

          <%!-- Main Content Panel --%>
          <div class="flex-1 flex flex-col overflow-hidden">
            <div class="flex-1 overflow-y-auto p-6">
              <%= case @current_view do %>
                <% "overview" -> %>
                  {render_overview(assigns)}
                <% "competitors" -> %>
                  {render_competitors(assigns)}
              <% end %>
            </div>
          </div>

          <%!-- Right Sidebar - Details Panel --%>
          <div class="w-80 bg-white border-l border-gray-200 overflow-y-auto flex-shrink-0">
            <div class="p-6">
              <h3 class="text-sm font-semibold text-gray-900 mb-4">Event Details</h3>
              <dl class="space-y-4">
                <%= if @event.description do %>
                  <div>
                    <dt class="text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Description
                    </dt>
                    <dd class="mt-1 text-sm text-gray-900">{@event.description}</dd>
                  </div>
                <% end %>

                <%= if @event.starts_at do %>
                  <div>
                    <dt class="text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Start Date
                    </dt>
                    <dd class="mt-1 text-sm text-gray-900">
                      {Calendar.strftime(@event.starts_at, "%B %d, %Y at %I:%M %p")}
                    </dd>
                  </div>
                <% end %>

                <%= if @event.ends_at do %>
                  <div>
                    <dt class="text-xs font-medium text-gray-500 uppercase tracking-wider">
                      End Date
                    </dt>
                    <dd class="mt-1 text-sm text-gray-900">
                      {Calendar.strftime(@event.ends_at, "%B %d, %Y at %I:%M %p")}
                    </dd>
                  </div>
                <% end %>

                <%= if @event.order_code do %>
                  <div>
                    <dt class="text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Order Code
                    </dt>
                    <dd class="mt-1 text-sm text-gray-900 font-mono">{@event.order_code}</dd>
                  </div>
                <% end %>

                <%= if @event.tax_rate_basis_points do %>
                  <div>
                    <dt class="text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Tax Rate
                    </dt>
                    <dd class="mt-1 text-sm text-gray-900">
                      {@event.tax_rate_basis_points / 100}%
                    </dd>
                  </div>
                <% end %>

                <%= if @event.storage_directory do %>
                  <div>
                    <dt class="text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Storage Directory
                    </dt>
                    <dd class="mt-1 text-xs text-gray-900 font-mono break-all">
                      {@event.storage_directory}
                    </dd>
                  </div>
                <% end %>
              </dl>
            </div>
          </div>
        </div>
      </div>
    </Layouts.admin>
    """
  end

  defp render_overview(assigns) do
    ~H"""
    <div class="max-w-4xl">
      <h2 class="text-2xl font-bold text-gray-900 mb-6">Event Overview</h2>

      <div class="grid grid-cols-1 md:grid-cols-3 gap-6 mb-8">
        <div class="bg-white rounded-lg border border-gray-200 p-6">
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm font-medium text-gray-500">Status</p>
              <p class="text-lg font-semibold text-gray-900 mt-2 capitalize">
                {Phoenix.Naming.humanize(@event.status)}
              </p>
            </div>
            <div class="p-3 bg-green-50 rounded-lg">
              <.icon name="hero-check-circle" class="w-6 h-6 text-green-600" />
            </div>
          </div>
        </div>

        <div class="bg-white rounded-lg border border-gray-200 p-6">
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm font-medium text-gray-500">Order Code</p>
              <p class="text-xl font-mono font-bold text-gray-900 mt-2">
                {@event.order_code || "-"}
              </p>
            </div>
            <div class="p-3 bg-purple-50 rounded-lg">
              <.icon name="hero-ticket" class="w-6 h-6 text-purple-600" />
            </div>
          </div>
        </div>

        <div class="bg-white rounded-lg border border-gray-200 p-6">
          <div class="flex items-center justify-between">
            <div>
              <p class="text-sm font-medium text-gray-500">Photos</p>
              <p class="text-3xl font-bold text-gray-900 mt-2">
                {@photo_counts.ready}
              </p>
            </div>
            <div class="p-3 bg-blue-50 rounded-lg">
              <.icon name="hero-photo" class="w-6 h-6 text-blue-600" />
            </div>
          </div>
        </div>
      </div>

      <%= if @event.description do %>
        <div class="bg-white rounded-lg border border-gray-200 p-6 mb-6">
          <h3 class="text-lg font-semibold text-gray-900 mb-3">Description</h3>
          <p class="text-gray-700">{@event.description}</p>
        </div>
      <% end %>

      <%!-- Ingestion Section --%>
      <div class="bg-white rounded-lg border border-gray-200 p-6">
        <h3 class="text-lg font-semibold text-gray-900 mb-4">Ingestion</h3>

        <div class="space-y-3 text-sm">
          <div class="flex justify-between items-center">
            <span class="text-gray-500">Storage Path</span>
            <code class="text-xs bg-gray-100 px-2 py-1 rounded font-mono">
              {@event.storage_directory || "Not configured"}
            </code>
          </div>

          <div class="flex justify-between items-center">
            <span class="text-gray-500">Photos</span>
            <span>
              <span class="text-green-600 font-medium">{@photo_counts.ready}</span> ready,
              <span class="text-blue-600">{@photo_counts.processing}</span> processing,
              <span class="text-red-600">{@photo_counts.error}</span> errors
            </span>
          </div>
        </div>

        <div class="mt-4 pt-4 border-t border-gray-100">
          <.button
            phx-click="scan_now"
            disabled={is_nil(@event.storage_directory)}
            size="small"
            variant="primary"
          >
            <.icon name="hero-magnifying-glass" class="w-4 h-4 mr-1" />
            Scan Now
          </.button>
          <%= if is_nil(@event.storage_directory) do %>
            <p class="text-xs text-gray-500 mt-2">
              Set a storage directory in the event settings to enable scanning.
            </p>
          <% end %>
        </div>
      </div>
    </div>
    """
  end

  defp render_competitors(assigns) do
    ~H"""
    <div class="max-w-4xl">
      <h2 class="text-2xl font-bold text-gray-900 mb-6">Competitor Roster</h2>

      <div class="text-center py-16 bg-white rounded-lg border-2 border-dashed border-gray-300">
        <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-gray-100 mb-4">
          <.icon name="hero-users" class="w-8 h-8 text-gray-400" />
        </div>
        <h3 class="text-lg font-semibold text-gray-900 mb-2">Coming Soon</h3>
        <p class="text-gray-500 max-w-md mx-auto">
          Competitor roster management features will be available in a future update
        </p>
      </div>
    </div>
    """
  end

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    event = Ash.get!(PhotoFinish.Events.Event, id)

    {:ok,
     socket
     |> assign(:page_title, event.name)
     |> assign(:current_view, "overview")
     |> assign(:photo_counts, load_photo_counts(id))
     |> assign(:event, event)}
  end

  defp load_photo_counts(event_id) do
    photos =
      Ash.read!(Photo)
      |> Enum.filter(&(&1.event_id == event_id))

    %{
      ready: Enum.count(photos, &(&1.status == :ready)),
      processing: Enum.count(photos, &(&1.status in [:discovered, :processing])),
      error: Enum.count(photos, &(&1.status == :error))
    }
  end

  @impl true
  def handle_event("select_view", %{"view" => view}, socket) do
    {:noreply, assign(socket, :current_view, view)}
  end

  @impl true
  def handle_event("scan_now", _params, socket) do
    case PhotoFinish.Ingestion.scan_event(socket.assigns.event.id) do
      {:ok, result} ->
        socket =
          socket
          |> put_flash(
            :info,
            "Scan complete. Found #{result.photos_new} new photos, #{result.photos_skipped} skipped."
          )
          |> assign(:photo_counts, load_photo_counts(socket.assigns.event.id))

        {:noreply, socket}

      {:error, :directory_not_found} ->
        {:noreply, put_flash(socket, :error, "Storage directory not found.")}

      {:error, reason} ->
        {:noreply, put_flash(socket, :error, "Scan failed: #{inspect(reason)}")}
    end
  end
end
