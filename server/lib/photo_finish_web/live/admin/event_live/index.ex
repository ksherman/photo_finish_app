defmodule PhotoFinishWeb.Admin.EventLive.Index do
  use PhotoFinishWeb, :live_view

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.admin flash={@flash}>
      <%!-- Top Toolbar --%>
      <div class="bg-white border-b border-gray-200 px-6 py-4">
        <div class="flex items-center justify-between">
          <div>
            <h1 class="text-2xl font-bold text-gray-900">Events</h1>
            <p class="text-sm text-gray-500 mt-1">Manage your photography events</p>
          </div>
          <div class="flex items-center gap-3">
            <.button_link navigate={~p"/admin/events/new"} variant="primary" size="large">
              <.icon name="hero-plus" class="w-5 h-5 mr-2" />
              New Event
            </.button_link>
          </div>
        </div>
      </div>

      <%!-- Main Content --%>
      <div class="p-6">
        <%= if @events_count == 0 do %>
          <%!-- Empty State --%>
          <div class="text-center py-12">
            <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-gray-100 mb-4">
              <.icon name="hero-calendar" class="w-8 h-8 text-gray-400" />
            </div>
            <h3 class="text-lg font-semibold text-gray-900 mb-2">No events yet</h3>
            <p class="text-gray-500 mb-6">Get started by creating your first event</p>
            <.button_link navigate={~p"/admin/events/new"} variant="primary">
              <.icon name="hero-plus" class="w-4 h-4 mr-2" />
              Create Your First Event
            </.button_link>
          </div>
        <% else %>
          <%!-- Events Grid --%>
          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6" id="events" phx-update="stream">
            <div
              :for={{id, event} <- @streams.events}
              id={id}
              class="bg-white rounded-lg border border-gray-200 hover:border-blue-400 hover:shadow-md transition-all cursor-pointer group"
              phx-click={JS.navigate(~p"/admin/events/#{event}")}
            >
              <%!-- Card Header --%>
              <div class="p-6 border-b border-gray-100">
                <div class="flex items-start justify-between">
                  <div class="flex-1 min-w-0">
                    <h3 class="text-lg font-semibold text-gray-900 truncate group-hover:text-blue-600 transition-colors">
                      {event.name}
                    </h3>
                    <%= if event.description do %>
                      <p class="text-sm text-gray-500 mt-1 line-clamp-2">{event.description}</p>
                    <% end %>
                  </div>
                  <.badge color={if event.status == :active, do: "success", else: "neutral"}>
                    {Phoenix.Naming.humanize(event.status)}
                  </.badge>
                </div>
              </div>

              <%!-- Card Body --%>
              <div class="p-6 space-y-3">
                <%= if event.starts_at do %>
                  <div class="flex items-center text-sm">
                    <.icon name="hero-calendar" class="w-4 h-4 text-gray-400 mr-2" />
                    <span class="text-gray-600">
                      {Calendar.strftime(event.starts_at, "%B %d, %Y")}
                    </span>
                  </div>
                <% end %>

                <%= if event.order_code do %>
                  <div class="flex items-center text-sm">
                    <.icon name="hero-ticket" class="w-4 h-4 text-gray-400 mr-2" />
                    <span class="text-gray-600">Order Code: {event.order_code}</span>
                  </div>
                <% end %>

                <%= if event.slug do %>
                  <div class="flex items-center text-sm">
                    <.icon name="hero-link" class="w-4 h-4 text-gray-400 mr-2" />
                    <span class="text-gray-600 font-mono text-xs">{event.slug}</span>
                  </div>
                <% end %>
              </div>

              <%!-- Card Footer --%>
              <div class="px-6 py-3 bg-gray-50 border-t border-gray-100 flex items-center justify-end gap-2">
                <.button
                  type="button"
                  size="small"
                  variant="outline"
                  phx-click={JS.navigate(~p"/admin/events/#{event}/edit")}
                >
                  <.icon name="hero-pencil" class="w-4 h-4 mr-1" />
                  Edit
                </.button>
                <.button
                  type="button"
                  size="small"
                  variant="outline"
                  phx-click={JS.push("delete", value: %{id: event.id}) |> hide("##{id}")}
                  data-confirm="Are you sure you want to delete this event?"
                >
                  <.icon name="hero-trash" class="w-4 h-4 mr-1" />
                  Delete
                </.button>
              </div>
            </div>
          </div>
        <% end %>
      </div>
    </Layouts.admin>
    """
  end

  @impl true
  def mount(_params, _session, socket) do
    events = Ash.read!(PhotoFinish.Events.Event)

    {:ok,
     socket
     |> assign(:page_title, "Events")
     |> assign(:events_count, length(events))
     |> stream(:events, events)}
  end

  @impl true
  def handle_event("delete", %{"id" => id}, socket) do
    event = Ash.get!(PhotoFinish.Events.Event, id)
    Ash.destroy!(event)

    {:noreply,
     socket
     |> stream_delete(:events, event)
     |> assign(:events_count, socket.assigns.events_count - 1)}
  end
end
