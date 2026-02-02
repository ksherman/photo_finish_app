defmodule PhotoFinishWeb.ViewerLive.Home do
  @moduledoc """
  Public viewer home page with competitor search.
  Families use this to search for their competitor by name or number.
  """
  use PhotoFinishWeb, :live_view

  require Ash.Query

  alias PhotoFinish.Events.Event
  alias PhotoFinish.Viewer.Search

  @impl true
  def mount(_params, _session, socket) do
    # Get the first active event (for MVP)
    event = get_active_event()

    socket =
      socket
      |> assign(:event, event)
      |> assign(:query, "")
      |> assign(:results, [])

    {:ok, socket}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="min-h-screen bg-gray-100">
      <!-- Header -->
      <header class="bg-white shadow-sm">
        <div class="max-w-4xl mx-auto px-4 py-4">
          <h1 class="text-xl font-bold text-gray-900">PhotoFinish</h1>
        </div>
      </header>

      <!-- Main Content -->
      <main class="max-w-4xl mx-auto px-4 py-12">
        <div class="text-center mb-12">
          <%= if @event do %>
            <h2 class="text-3xl font-bold text-gray-900 mb-2"><%= @event.name %></h2>
          <% else %>
            <h2 class="text-3xl font-bold text-gray-900 mb-2">Photo Viewer</h2>
          <% end %>
        </div>

        <!-- Search Box -->
        <div class="max-w-xl mx-auto">
          <form phx-change="search" phx-submit="search">
            <div class="relative">
              <input
                type="text"
                name="query"
                value={@query}
                placeholder="Search by name or number..."
                phx-debounce="300"
                autocomplete="off"
                class="w-full px-6 py-4 text-lg rounded-full border-2 border-gray-200 focus:border-indigo-500 focus:ring-0 shadow-sm"
              />
              <div class="absolute right-4 top-1/2 -translate-y-1/2">
                <.icon name="hero-magnifying-glass" class="w-6 h-6 text-gray-400" />
              </div>
            </div>
          </form>

          <!-- Results -->
          <%= if @query != "" do %>
            <div class="mt-4 bg-white rounded-xl shadow-lg overflow-hidden">
              <%= if @results == [] do %>
                <div class="p-6 text-center text-gray-500">
                  No competitors found for "<%= @query %>"
                </div>
              <% else %>
                <ul class="divide-y divide-gray-100">
                  <%= for result <- @results do %>
                    <li>
                      <.link
                        navigate={~p"/view/competitor/#{result.id}"}
                        class="block px-6 py-4 hover:bg-gray-50 flex items-center justify-between"
                      >
                        <div>
                          <div class="font-medium text-gray-900">
                            <span class="font-mono mr-2"><%= result.competitor_number %></span>
                            <%= result.first_name %> <%= result.last_name %>
                          </div>
                          <div class="text-sm text-gray-500">
                            Session <%= result.session %>
                          </div>
                        </div>
                        <div class="flex items-center gap-2 text-gray-500">
                          <span><%= result.photo_count %> photos</span>
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
      </main>
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

  defp get_active_event do
    Event
    |> Ash.Query.filter(status == :active)
    |> Ash.Query.limit(1)
    |> Ash.read!()
    |> List.first()
  end
end
