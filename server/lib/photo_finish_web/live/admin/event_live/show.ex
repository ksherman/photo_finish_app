defmodule PhotoFinishWeb.Admin.EventLive.Show do
  use PhotoFinishWeb, :live_view

  alias PhotoFinish.Events.FolderGenerator
  alias PhotoFinish.Photos.Photo

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
              <h2 class="text-lg font-semibold text-gray-900">Photos</h2>
            </div>
            <div class="p-6">
              <div class="grid grid-cols-3 gap-4 mb-6">
                <div class="text-center p-4 bg-green-50 rounded-lg">
                  <p class="text-3xl font-bold text-green-700">{@photo_counts.ready}</p>
                  <p class="text-sm text-green-600 mt-1">Ready</p>
                </div>
                <div class="text-center p-4 bg-blue-50 rounded-lg">
                  <p class="text-3xl font-bold text-blue-700">{@photo_counts.processing}</p>
                  <p class="text-sm text-blue-600 mt-1">Processing</p>
                </div>
                <div class="text-center p-4 bg-red-50 rounded-lg">
                  <p class="text-3xl font-bold text-red-700">{@photo_counts.error}</p>
                  <p class="text-sm text-red-600 mt-1">Errors</p>
                </div>
              </div>

              <div class="flex items-center justify-between pt-4 border-t border-gray-100">
                <div class="text-sm text-gray-500">
                  Scan the storage directory for new photos
                </div>
                <.button
                  phx-click="scan_now"
                  disabled={is_nil(@event.storage_root)}
                  size="small"
                >
                  <.icon name="hero-magnifying-glass" class="w-4 h-4 mr-1" /> Scan Now
                </.button>
              </div>
              <%= if is_nil(@event.storage_root) do %>
                <p class="text-xs text-amber-600 mt-2">
                  Set a storage root in the event settings to enable scanning.
                </p>
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

    gym_letters = format_gym_letters(event.num_gyms)
    total_sessions = event.num_gyms * event.sessions_per_gym

    {:ok,
     socket
     |> assign(:page_title, event.name)
     |> assign(:event, event)
     |> assign(:gym_letters, gym_letters)
     |> assign(:total_sessions, total_sessions)
     |> assign(:photo_counts, load_photo_counts(id))}
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

    %{
      ready: Enum.count(photos, &(&1.status == :ready)),
      processing: Enum.count(photos, &(&1.status in [:discovered, :processing])),
      error: Enum.count(photos, &(&1.status == :error))
    }
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
