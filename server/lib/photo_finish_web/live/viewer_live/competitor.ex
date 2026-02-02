defmodule PhotoFinishWeb.ViewerLive.Competitor do
  @moduledoc """
  Public viewer page for displaying photos of a specific competitor.
  Shows a photo grid with lightbox functionality for viewing larger images.
  """
  use PhotoFinishWeb, :live_view

  require Ash.Query

  alias PhotoFinish.Events.EventCompetitor
  alias PhotoFinish.Photos.Photo

  @impl true
  def mount(%{"id" => id}, _session, socket) do
    event_competitor = load_event_competitor(id)
    photos = load_photos(id)

    socket =
      socket
      |> assign(:event_competitor, event_competitor)
      |> assign(:photos, photos)
      |> assign(:lightbox_photo, nil)
      |> assign(:lightbox_index, nil)

    {:ok, socket}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="min-h-screen bg-gray-100">
      <!-- Header -->
      <header class="bg-white shadow-sm sticky top-0 z-10">
        <div class="max-w-6xl mx-auto px-4 py-4 flex items-center gap-4">
          <.link navigate={~p"/view"} class="text-gray-500 hover:text-gray-700">
            <.icon name="hero-arrow-left" class="w-6 h-6" />
          </.link>
          <div>
            <h1 class="text-lg font-bold text-gray-900">
              <%= @event_competitor.display_name || "Competitor #{@event_competitor.competitor_number}" %>
            </h1>
            <p class="text-sm text-gray-500"><%= length(@photos) %> photos</p>
          </div>
        </div>
      </header>

      <!-- Photo Grid -->
      <main class="max-w-6xl mx-auto px-4 py-6">
        <div class="grid grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-2">
          <%= for {photo, idx} <- Enum.with_index(@photos) do %>
            <button
              phx-click="open_lightbox"
              phx-value-index={idx}
              class="aspect-[3/2] bg-gray-200 rounded overflow-hidden hover:opacity-90 transition"
            >
              <img
                src={~p"/view/photos/thumbnail/#{photo.id}"}
                loading="lazy"
                class="w-full h-full object-cover"
              />
            </button>
          <% end %>
        </div>

        <%= if @photos == [] do %>
          <div class="text-center py-12 text-gray-500">
            No photos available yet
          </div>
        <% end %>
      </main>

      <!-- Lightbox -->
      <%= if @lightbox_photo do %>
        <div
          class="fixed inset-0 bg-black/90 z-50 flex items-center justify-center"
          phx-window-keydown="lightbox_key"
        >
          <!-- Close -->
          <button
            phx-click="close_lightbox"
            class="absolute top-4 right-4 text-white/70 hover:text-white"
          >
            <.icon name="hero-x-mark" class="w-8 h-8" />
          </button>

          <!-- Previous -->
          <%= if @lightbox_index > 0 do %>
            <button
              phx-click="lightbox_prev"
              class="absolute left-4 top-1/2 -translate-y-1/2 text-white/70 hover:text-white p-2"
            >
              <.icon name="hero-chevron-left" class="w-10 h-10" />
            </button>
          <% end %>

          <!-- Image -->
          <div class="max-w-4xl max-h-[80vh] px-16">
            <img
              src={~p"/view/photos/preview/#{@lightbox_photo.id}"}
              class="max-w-full max-h-[80vh] object-contain"
            />
          </div>

          <!-- Next -->
          <%= if @lightbox_index < length(@photos) - 1 do %>
            <button
              phx-click="lightbox_next"
              class="absolute right-4 top-1/2 -translate-y-1/2 text-white/70 hover:text-white p-2"
            >
              <.icon name="hero-chevron-right" class="w-10 h-10" />
            </button>
          <% end %>

          <!-- Counter -->
          <div class="absolute bottom-4 left-1/2 -translate-x-1/2 text-white/70">
            <%= @lightbox_index + 1 %> of <%= length(@photos) %>
          </div>
        </div>
      <% end %>
    </div>
    """
  end

  @impl true
  def handle_event("open_lightbox", %{"index" => index}, socket) do
    index = String.to_integer(index)
    photo = Enum.at(socket.assigns.photos, index)
    {:noreply, socket |> assign(:lightbox_photo, photo) |> assign(:lightbox_index, index)}
  end

  def handle_event("close_lightbox", _params, socket) do
    {:noreply, socket |> assign(:lightbox_photo, nil) |> assign(:lightbox_index, nil)}
  end

  def handle_event("lightbox_prev", _params, socket) do
    new_index = max(0, socket.assigns.lightbox_index - 1)
    photo = Enum.at(socket.assigns.photos, new_index)
    {:noreply, socket |> assign(:lightbox_photo, photo) |> assign(:lightbox_index, new_index)}
  end

  def handle_event("lightbox_next", _params, socket) do
    max_index = length(socket.assigns.photos) - 1
    new_index = min(max_index, socket.assigns.lightbox_index + 1)
    photo = Enum.at(socket.assigns.photos, new_index)
    {:noreply, socket |> assign(:lightbox_photo, photo) |> assign(:lightbox_index, new_index)}
  end

  def handle_event("lightbox_key", %{"key" => "Escape"}, socket) do
    handle_event("close_lightbox", %{}, socket)
  end

  def handle_event("lightbox_key", %{"key" => "ArrowLeft"}, socket) do
    if socket.assigns.lightbox_index && socket.assigns.lightbox_index > 0 do
      handle_event("lightbox_prev", %{}, socket)
    else
      {:noreply, socket}
    end
  end

  def handle_event("lightbox_key", %{"key" => "ArrowRight"}, socket) do
    if socket.assigns.lightbox_index && socket.assigns.lightbox_index < length(socket.assigns.photos) - 1 do
      handle_event("lightbox_next", %{}, socket)
    else
      {:noreply, socket}
    end
  end

  def handle_event("lightbox_key", _params, socket) do
    {:noreply, socket}
  end

  defp load_event_competitor(id) do
    Ash.get!(EventCompetitor, id)
  end

  defp load_photos(event_competitor_id) do
    Photo
    |> Ash.Query.filter(event_competitor_id == ^event_competitor_id and status == :ready)
    |> Ash.Query.sort(:filename)
    |> Ash.read!()
  end
end
