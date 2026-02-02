defmodule PhotoFinishWeb.Admin.FolderLive.Associate do
  @moduledoc """
  LiveView for associating photo folders (source_folder) to event competitors.

  The UI has two modes:
  1. Navigation mode (path length < 4): Show hierarchy navigation buttons (gym -> session -> group -> apparatus)
  2. Association mode (path length == 4): Show split view with folders on left, competitors on right
  """
  use PhotoFinishWeb, :live_view

  alias PhotoFinish.Events.Event
  alias PhotoFinish.Events.FolderAssociation
  alias PhotoFinish.Photos.LocationBrowser

  @impl true
  def mount(%{"event_id" => event_id}, _session, socket) do
    event = Ash.get!(Event, event_id)

    socket =
      socket
      |> assign(:event, event)
      |> assign(:path, [])
      |> assign(:children, LocationBrowser.get_children(event_id, []))
      |> assign(:folders, [])
      |> assign(:competitors, [])
      |> assign(:selected_folder, nil)
      |> assign(:assignments, %{})

    {:ok, socket}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.admin flash={@flash}>
      <div class="min-h-screen bg-gray-50">
        <%!-- Header --%>
        <div class="bg-white border-b border-gray-200 px-6 py-4">
          <div class="max-w-6xl mx-auto">
            <div class="flex items-center gap-4">
              <.button_link
                navigate={~p"/admin/events/#{@event.id}"}
                variant="outline"
                color="natural"
                size="small"
              >
                <.icon name="hero-arrow-left" class="w-4 h-4" />
              </.button_link>
              <div>
                <h1 class="text-xl font-bold text-gray-900">Associate Folders to Competitors</h1>
                <p class="text-sm text-gray-500">{@event.name}</p>
              </div>
            </div>
          </div>
        </div>

        <%!-- Main Content --%>
        <div class="max-w-6xl mx-auto py-8 px-6">
          <%!-- Breadcrumb --%>
          <nav class="mb-6">
            <ol class="flex items-center gap-2 text-sm">
              <li>
                <button
                  phx-click="nav"
                  phx-value-level="-1"
                  class={[
                    "hover:text-blue-600",
                    @path == [] && "font-semibold text-gray-900",
                    @path != [] && "text-blue-600"
                  ]}
                >
                  Event
                </button>
              </li>
              <%= for {item, idx} <- Enum.with_index(@path) do %>
                <li class="text-gray-400">/</li>
                <li>
                  <button
                    phx-click="nav"
                    phx-value-level={idx}
                    class={[
                      "hover:text-blue-600",
                      idx == length(@path) - 1 && "font-semibold text-gray-900",
                      idx != length(@path) - 1 && "text-blue-600"
                    ]}
                  >
                    {LocationBrowser.format_value_at_index(item, idx)}
                  </button>
                </li>
              <% end %>
            </ol>
          </nav>

          <%= if length(@path) < 4 do %>
            <%!-- Navigation Mode: Show children --%>
            <%= if @children != [] do %>
              <p class="text-xs text-gray-500 mb-4">
                Select a {LocationBrowser.level_label(LocationBrowser.current_level(@path))}
              </p>
              <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
                <%= for child <- @children do %>
                  <% current_level = LocationBrowser.current_level(@path) %>
                  <button
                    phx-click="drill"
                    phx-value-name={child.name}
                    class="p-4 bg-white rounded-lg border border-gray-200 hover:border-blue-300 hover:bg-blue-50 transition-colors text-left"
                  >
                    <div class="font-medium text-gray-900">
                      {LocationBrowser.format_value(current_level, child.name)}
                    </div>
                    <div class="text-sm text-gray-500">{child.count} photos</div>
                  </button>
                <% end %>
              </div>
            <% else %>
              <div class="text-center py-12">
                <p class="text-gray-500">No photos found at this location.</p>
              </div>
            <% end %>
          <% else %>
            <%!-- Association Mode: Split view --%>
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-8">
              <%!-- Left: Unassigned Folders --%>
              <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
                <div class="px-4 py-3 bg-gray-50 border-b border-gray-200">
                  <h2 class="font-semibold text-gray-900">Unassigned Folders</h2>
                  <p class="text-xs text-gray-500 mt-1">
                    Click a folder to select it, then click a competitor to assign
                  </p>
                </div>
                <div class="p-4 space-y-2 max-h-96 overflow-y-auto">
                  <%= for folder <- @folders do %>
                    <button
                      phx-click="select_folder"
                      phx-value-folder={folder.source_folder}
                      disabled={Map.has_key?(@assignments, folder.source_folder)}
                      class={[
                        "w-full p-3 rounded-lg text-left flex justify-between items-center transition-colors",
                        @selected_folder == folder.source_folder && "bg-blue-100 border-2 border-blue-500",
                        @selected_folder != folder.source_folder && !Map.has_key?(@assignments, folder.source_folder) && "bg-gray-50 border border-gray-200 hover:border-gray-300 hover:bg-gray-100",
                        Map.has_key?(@assignments, folder.source_folder) && "opacity-50 bg-green-50 border border-green-200 cursor-not-allowed"
                      ]}
                    >
                      <span class="font-mono text-sm">{folder.source_folder}</span>
                      <span class="text-sm text-gray-500">{folder.photo_count} photos</span>
                    </button>
                  <% end %>
                  <%= if @folders == [] do %>
                    <p class="text-gray-500 italic text-center py-4">No unassigned folders</p>
                  <% end %>
                </div>
              </div>

              <%!-- Right: Competitors --%>
              <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
                <div class="px-4 py-3 bg-gray-50 border-b border-gray-200">
                  <h2 class="font-semibold text-gray-900">
                    Session {Enum.at(@path, 1)} Competitors
                  </h2>
                  <p class="text-xs text-gray-500 mt-1">
                    <%= if @selected_folder do %>
                      Click a competitor to assign folder "<span class="font-mono">{@selected_folder}</span>"
                    <% else %>
                      Select a folder first
                    <% end %>
                  </p>
                </div>
                <div class="p-4 space-y-1 max-h-96 overflow-y-auto">
                  <%= for competitor <- @competitors do %>
                    <button
                      phx-click="assign"
                      phx-value-competitor-id={competitor.id}
                      disabled={@selected_folder == nil}
                      class={[
                        "w-full p-2 rounded text-left flex items-center gap-3 transition-colors",
                        @selected_folder && "hover:bg-blue-50 cursor-pointer",
                        !@selected_folder && "opacity-50 cursor-not-allowed"
                      ]}
                    >
                      <span class="font-mono text-sm w-16 text-gray-600">{competitor.competitor_number}</span>
                      <span class="text-gray-900">{competitor.display_name}</span>
                    </button>
                  <% end %>
                  <%= if @competitors == [] do %>
                    <p class="text-gray-500 italic text-center py-4">No competitors for this session</p>
                  <% end %>
                </div>
              </div>
            </div>

            <%!-- Pending Assignments --%>
            <%= if map_size(@assignments) > 0 do %>
              <div class="mt-8 bg-white rounded-lg border border-gray-200 overflow-hidden">
                <div class="px-4 py-3 bg-gray-50 border-b border-gray-200 flex items-center justify-between">
                  <div>
                    <h3 class="font-semibold text-gray-900">Pending Assignments</h3>
                    <p class="text-xs text-gray-500">{map_size(@assignments)} assignment(s) ready to save</p>
                  </div>
                  <.button
                    phx-click="save_assignments"
                    size="small"
                    variant="default"
                    color="primary"
                  >
                    <.icon name="hero-check" class="w-4 h-4 mr-1" /> Save All Assignments
                  </.button>
                </div>
                <div class="p-4 space-y-2">
                  <%= for {folder, competitor} <- @assignments do %>
                    <div class="flex items-center justify-between p-2 bg-gray-50 rounded">
                      <div class="flex items-center gap-2">
                        <span class="font-mono text-sm text-gray-700">{folder}</span>
                        <.icon name="hero-arrow-right" class="w-4 h-4 text-gray-400" />
                        <span class="font-mono text-sm text-gray-500">{competitor.competitor_number}</span>
                        <span class="text-gray-900">{competitor.display_name}</span>
                      </div>
                      <button
                        phx-click="unassign"
                        phx-value-folder={folder}
                        class="text-red-600 hover:text-red-800 text-sm"
                      >
                        Remove
                      </button>
                    </div>
                  <% end %>
                </div>
              </div>
            <% end %>
          <% end %>
        </div>
      </div>
    </Layouts.admin>
    """
  end

  @impl true
  def handle_event("drill", %{"name" => name}, socket) do
    new_path = socket.assigns.path ++ [name]

    {:noreply,
     socket
     |> assign(:path, new_path)
     |> assign(:selected_folder, nil)
     |> load_data_for_path(new_path)}
  end

  def handle_event("nav", %{"level" => level}, socket) do
    level = String.to_integer(level)
    new_path = if level < 0, do: [], else: Enum.take(socket.assigns.path, level + 1)

    {:noreply,
     socket
     |> assign(:path, new_path)
     |> assign(:selected_folder, nil)
     |> assign(:assignments, %{})
     |> load_data_for_path(new_path)}
  end

  def handle_event("select_folder", %{"folder" => folder}, socket) do
    {:noreply, assign(socket, :selected_folder, folder)}
  end

  def handle_event("assign", %{"competitor-id" => competitor_id}, socket) do
    folder = socket.assigns.selected_folder
    competitor = Enum.find(socket.assigns.competitors, &(&1.id == competitor_id))
    assignments = Map.put(socket.assigns.assignments, folder, competitor)

    {:noreply,
     socket
     |> assign(:assignments, assignments)
     |> assign(:selected_folder, nil)}
  end

  def handle_event("unassign", %{"folder" => folder}, socket) do
    assignments = Map.delete(socket.assigns.assignments, folder)
    {:noreply, assign(socket, :assignments, assignments)}
  end

  def handle_event("save_assignments", _params, socket) do
    event_id = socket.assigns.event.id

    results =
      for {folder, competitor} <- socket.assigns.assignments do
        FolderAssociation.assign_folder(event_id, folder, competitor.id)
      end

    total = Enum.sum(for {:ok, count} <- results, do: count)

    socket =
      socket
      |> assign(:assignments, %{})
      |> put_flash(:info, "Assigned #{total} photos to competitors")
      |> load_data_for_path(socket.assigns.path)

    {:noreply, socket}
  end

  defp load_data_for_path(socket, path) when length(path) < 4 do
    children = LocationBrowser.get_children(socket.assigns.event.id, path)

    socket
    |> assign(:children, children)
    |> assign(:folders, [])
    |> assign(:competitors, [])
  end

  defp load_data_for_path(socket, [gym, session, group_name, apparatus]) do
    event_id = socket.assigns.event.id

    folders =
      FolderAssociation.list_unassigned_folders(event_id, %{
        gym: gym,
        session: session,
        group_name: group_name,
        apparatus: apparatus
      })

    competitors = FolderAssociation.list_session_event_competitors(event_id, session)

    socket
    |> assign(:children, [])
    |> assign(:folders, folders)
    |> assign(:competitors, competitors)
  end
end
