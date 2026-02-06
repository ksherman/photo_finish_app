defmodule PhotoFinishWeb.ViewerLive.Home do
  @moduledoc """
  Public viewer home page with competitor search.
  Families use this to search for their competitor by name or number.
  """
  use PhotoFinishWeb, :live_view

  require Ash.Query

  alias PhotoFinish.Events.Event
  alias PhotoFinish.Photos.Photo
  alias PhotoFinish.Viewer.Search

  @showcase_photo_count 24

  @impl true
  def mount(params, _session, socket) do
    case params do
      %{"event_id" => event_id} ->
        event = Ash.get!(Event, event_id)
        showcase_photos = load_showcase_photos(event.id)
        {row1, row2, row3} = split_into_rows(showcase_photos)

        socket =
          socket
          |> assign(:event, event)
          |> assign(:mode, :search)
          |> assign(:query, "")
          |> assign(:results, [])
          |> assign(:showcase_row1, row1)
          |> assign(:showcase_row2, row2)
          |> assign(:showcase_row3, row3)

        {:ok, socket}

      _ ->
        events = list_active_events()

        # If only one active event, redirect directly
        case events do
          [single_event] ->
            {:ok, push_navigate(socket, to: ~p"/viewer/#{single_event.id}")}

          _ ->
            socket =
              socket
              |> assign(:events, events)
              |> assign(:mode, :pick_event)

            {:ok, socket}
        end
    end
  end

  @impl true
  def render(%{mode: :pick_event} = assigns) do
    ~H"""
    <div class="h-screen bg-gray-900 flex items-center justify-center">
      <div class="max-w-xl w-full mx-auto px-4">
        <div class="bg-white/95 backdrop-blur-sm rounded-2xl shadow-2xl p-8">
          <div class="text-center mb-6">
            <h1 class="text-3xl font-bold text-gray-900 mb-1">PhotoFinish</h1>
            <p class="text-gray-500">Select an event</p>
          </div>

          <%= if @events == [] do %>
            <p class="text-center text-gray-500 py-4">No active events</p>
          <% else %>
            <div class="space-y-3">
              <%= for event <- @events do %>
                <.link
                  navigate={~p"/viewer/#{event.id}"}
                  class="block w-full p-4 rounded-xl border-2 border-gray-200 hover:border-indigo-500 hover:bg-indigo-50 transition text-left"
                >
                  <p class="font-semibold text-gray-900 text-lg">{event.name}</p>
                  <p :if={event.description} class="text-sm text-gray-500 mt-1">
                    {event.description}
                  </p>
                </.link>
              <% end %>
            </div>
          <% end %>
        </div>
      </div>
    </div>
    """
  end

  def render(%{mode: :search} = assigns) do
    ~H"""
    <style>
      @keyframes marquee-left {
        from { transform: translateX(0); }
        to { transform: translateX(-50%); }
      }
      @keyframes marquee-right {
        from { transform: translateX(-50%); }
        to { transform: translateX(0); }
      }
      .marquee-left {
        animation: marquee-left 300s linear infinite;
      }
      .marquee-right {
        animation: marquee-right 900s linear infinite;
      }
      .marquee-left-slow {
        animation: marquee-left 350s linear infinite;
      }
    </style>

    <div class="h-screen bg-gray-900 overflow-hidden relative">
      <%!-- Photo Showcase Background --%>
      <%= if @showcase_row1 != [] do %>
        <div class="absolute inset-0 flex flex-col gap-1">
          <%!-- Row 1: 20vh, scrolls left --%>
          <div class="h-[20vh] overflow-hidden">
            <div class="marquee-left flex gap-1 h-full w-max">
              <%= for photo <- List.flatten(List.duplicate(@showcase_row1, 6)) do %>
                <img
                  src={~p"/viewer/photos/thumbnail/#{photo.id}"}
                  class="h-full w-auto object-cover"
                />
              <% end %>
            </div>
          </div>

          <%!-- Row 2: 60vh, scrolls right --%>
          <div class="h-[60vh] overflow-hidden">
            <div class="marquee-right flex gap-1 h-full w-max">
              <%= for photo <- List.flatten(List.duplicate(@showcase_row2, 6)) do %>
                <img
                  src={~p"/viewer/photos/preview/#{photo.id}"}
                  class="h-full w-auto object-cover"
                />
              <% end %>
            </div>
          </div>

          <%!-- Row 3: 20vh, scrolls left slower --%>
          <div class="h-[20vh] overflow-hidden">
            <div class="marquee-left-slow flex gap-1 h-full w-max">
              <%= for photo <- List.flatten(List.duplicate(@showcase_row3, 6)) do %>
                <img
                  src={~p"/viewer/photos/thumbnail/#{photo.id}"}
                  class="h-full w-auto object-cover"
                />
              <% end %>
            </div>
          </div>
        </div>
      <% end %>

      <%!-- Search Overlay --%>
      <div class="absolute inset-x-0 top-[35%] -translate-y-1/2 z-10">
        <div class="max-w-xl mx-auto px-4">
          <div class="bg-white/95 backdrop-blur-sm rounded-2xl shadow-2xl p-8">
            <div class="text-center mb-6">
              <h1 class="text-3xl font-bold text-gray-900 mb-1">{@event.name}</h1>
              <p class="text-gray-500">Find your photos</p>
            </div>

            <form phx-change="search" phx-submit="search">
              <div class="relative">
                <input
                  type="text"
                  name="query"
                  value={@query}
                  placeholder="Search by name or number..."
                  phx-debounce="300"
                  autocomplete="off"
                  class="w-full px-6 py-4 text-lg rounded-full border-2 border-gray-200 focus:border-indigo-500 focus:ring-0"
                />
                <div class="absolute right-4 top-1/2 -translate-y-1/2">
                  <.icon name="hero-magnifying-glass" class="w-6 h-6 text-gray-400" />
                </div>
              </div>
            </form>

            <%!-- Results --%>
            <%= if @query != "" do %>
              <div class="mt-4 max-h-64 overflow-y-auto rounded-xl border border-gray-200">
                <%= if @results == [] do %>
                  <div class="p-6 text-center text-gray-500">
                    No competitors found for "{@query}"
                  </div>
                <% else %>
                  <ul class="divide-y divide-gray-100">
                    <%= for result <- @results do %>
                      <li>
                        <.link
                          navigate={~p"/viewer/#{@event.id}/competitor/#{result.id}"}
                          class="block px-6 py-4 hover:bg-gray-50 flex items-center justify-between"
                        >
                          <div>
                            <div class="font-medium text-gray-900">
                              <span class="font-mono mr-2">{result.competitor_number}</span>
                              {result.first_name} {result.last_name}
                            </div>
                            <div class="text-sm text-gray-500">
                              Session {result.session}
                            </div>
                          </div>
                          <div class="flex items-center gap-2 text-gray-500">
                            <span>{result.photo_count} photos</span>
                            <.icon name="hero-chevron-right" class="w-5 h-5" />
                          </div>
                        </.link>
                      </li>
                    <% end %>
                  </ul>
                <% end %>
              </div>
            <% end %>
          </div>
        </div>
      </div>
    </div>
    """
  end

  @impl true
  def handle_event("search", %{"query" => query}, socket) do
    socket = assign(socket, :query, query)

    if socket.assigns.event && String.length(String.trim(query)) > 0 do
      results = Search.search_event_competitors(socket.assigns.event.id, query)
      {:noreply, assign(socket, :results, results)}
    else
      {:noreply, assign(socket, :results, [])}
    end
  end

  defp list_active_events do
    Event
    |> Ash.Query.filter(status == :active)
    |> Ash.Query.sort(:name)
    |> Ash.read!()
  end

  defp load_showcase_photos(event_id) do
    Photo
    |> Ash.Query.filter(event_id == ^event_id and status == :ready)
    |> Ash.read!()
    |> Enum.shuffle()
    |> Enum.take(@showcase_photo_count)
  end

  defp split_into_rows(photos) do
    # Split photos into 3 rows: 25%, 50%, 25% distribution
    total = length(photos)
    row1_count = max(1, div(total, 4))
    row3_count = max(1, div(total, 4))
    row2_count = total - row1_count - row3_count

    {row1, rest} = Enum.split(photos, row1_count)
    {row2, row3} = Enum.split(rest, row2_count)

    {row1, row2, row3}
  end
end
