defmodule PhotoFinishWeb.Admin.EventLive.Show do
  use PhotoFinishWeb, :live_view

  alias PhotoFinishWeb.Admin.EventLive.Components.StructureBuilder
  alias PhotoFinish.Events.HierarchyLevel
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
              <.button_link navigate={~p"/admin/events"} variant="outline" size="small">
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
              <%= if @show_builder do %>
                <.button phx-click="close_builder_modal" size="small" variant="outline">
                  <.icon name="hero-x-mark" class="w-4 h-4 mr-1" /> Close Builder
                </.button>
              <% else %>
                <%= if @event.hierarchy_levels != [] do %>
                  <.button phx-click="start_builder" size="small" variant="primary">
                    <.icon name="hero-plus" class="w-4 h-4 mr-1" /> Build Structure
                  </.button>
                <% end %>
                <.button_link
                  navigate={~p"/admin/events/#{@event}/edit"}
                  size="small"
                  variant="outline"
                >
                  <.icon name="hero-pencil" class="w-4 h-4 mr-1" /> Edit Event
                </.button_link>
              <% end %>
            </div>
          </div>
        </div>

        <%= if @show_builder do %>
          <%!-- Structure Builder Modal Overlay --%>
          <div class="flex-1 overflow-hidden bg-white">
            <div class="h-full overflow-y-auto p-6">
              <.live_component
                module={StructureBuilder}
                id="structure-builder"
                event={@event}
                levels={@event.hierarchy_levels}
              />
            </div>
          </div>
        <% else %>
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
                    phx-value-view="structure"
                    class={[
                      "w-full flex items-center px-3 py-2 text-sm rounded-lg transition-colors",
                      @current_view == "structure" &&
                        "bg-blue-50 text-blue-700 font-medium",
                      @current_view != "structure" &&
                        "text-gray-700 hover:bg-gray-50"
                    ]}
                  >
                    <.icon name="hero-rectangle-stack" class="w-4 h-4 mr-2" /> Hierarchy Levels
                  </button>
                  <button
                    phx-click="select_view"
                    phx-value-view="nodes"
                    class={[
                      "w-full flex items-center px-3 py-2 text-sm rounded-lg transition-colors",
                      @current_view == "nodes" &&
                        "bg-blue-50 text-blue-700 font-medium",
                      @current_view != "nodes" &&
                        "text-gray-700 hover:bg-gray-50"
                    ]}
                  >
                    <.icon name="hero-folder" class="w-4 h-4 mr-2" /> Hierarchy Nodes
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
                  <% "structure" -> %>
                    {render_structure(assigns)}
                  <% "nodes" -> %>
                    {render_nodes(assigns)}
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
        <% end %>
      </div>

      <%!-- Level Edit Modal --%>
      <%= if @editing_level do %>
        <div
          class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
          phx-click="cancel_edit_level"
        >
          <div
            class="bg-white rounded-lg shadow-xl max-w-2xl w-full mx-4"
            phx-click="stop_propagation"
          >
            <div class="px-6 py-4 border-b border-gray-200">
              <h3 class="text-lg font-semibold text-gray-900">
                {if @editing_level[:id], do: "Edit Hierarchy Level", else: "Add Hierarchy Level"}
              </h3>
            </div>

            <.form for={@level_form} phx-submit="save_hierarchy_level" class="p-6 space-y-6">
              <div class="grid grid-cols-2 gap-4">
                <.input
                  field={@level_form[:level_name]}
                  type="text"
                  label="Level Name (Singular)"
                  placeholder="e.g., Gym, Session, Apparatus"
                  required
                />

                <.input
                  field={@level_form[:level_name_plural]}
                  type="text"
                  label="Plural Form"
                  placeholder="e.g., Gyms, Sessions, Apparatuses"
                  required
                />
              </div>

              <div class="grid grid-cols-2 gap-4">
                <div class="flex items-center">
                  <input
                    type="checkbox"
                    id="is_required"
                    name={@level_form[:is_required].name}
                    value="true"
                    checked={Phoenix.HTML.Form.input_value(@level_form, :is_required)}
                    class="h-4 w-4 text-blue-600 rounded border-gray-300 focus:ring-blue-500"
                  />
                  <label for="is_required" class="ml-2 text-sm text-gray-700">
                    Required level
                  </label>
                </div>

                <div class="flex items-center">
                  <input
                    type="checkbox"
                    id="allow_photos"
                    name={@level_form[:allow_photos].name}
                    value="true"
                    checked={Phoenix.HTML.Form.input_value(@level_form, :allow_photos)}
                    class="h-4 w-4 text-blue-600 rounded border-gray-300 focus:ring-blue-500"
                  />
                  <label for="allow_photos" class="ml-2 text-sm text-gray-700">
                    Allow photos at this level
                  </label>
                </div>
              </div>

              <div class="p-4 bg-gray-50 rounded-lg">
                <p class="text-xs text-gray-600">
                  <strong>Tip:</strong>
                  Typically, only the final level (e.g., Competitor) should allow photos.
                  Other levels are used for organization and navigation.
                </p>
              </div>

              <div class="flex items-center justify-end gap-3 pt-4 border-t">
                <.button type="button" phx-click="cancel_edit_level" variant="outline">
                  Cancel
                </.button>
                <.button type="submit" variant="primary">
                  <.icon name="hero-check" class="w-4 h-4 mr-1" />
                  {if @editing_level[:id], do: "Update Level", else: "Add Level"}
                </.button>
              </div>
            </.form>
          </div>
        </div>
      <% end %>
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
              <p class="text-sm font-medium text-gray-500">Hierarchy Levels</p>
              <p class="text-3xl font-bold text-gray-900 mt-2">
                {length(@event.hierarchy_levels)}
              </p>
            </div>
            <div class="p-3 bg-blue-50 rounded-lg">
              <.icon name="hero-rectangle-stack" class="w-6 h-6 text-blue-600" />
            </div>
          </div>
        </div>

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
                {@event.order_code || "—"}
              </p>
            </div>
            <div class="p-3 bg-purple-50 rounded-lg">
              <.icon name="hero-ticket" class="w-6 h-6 text-purple-600" />
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

  defp render_structure(assigns) do
    ~H"""
    <div class="max-w-5xl">
      <div class="flex items-center justify-between mb-6">
        <div>
          <h2 class="text-2xl font-bold text-gray-900">Hierarchy Levels</h2>
          <p class="text-sm text-gray-500 mt-1">
            Define the organizational structure for this event
          </p>
        </div>
        <div class="flex items-center gap-2">
          <%= if @event.hierarchy_levels == [] do %>
            <.button phx-click="generate_standard_hierarchy" variant="primary">
              <.icon name="hero-sparkles" class="w-4 h-4 mr-2" /> Generate Standard
            </.button>
          <% else %>
            <.button phx-click="add_hierarchy_level" variant="outline" size="small">
              <.icon name="hero-plus" class="w-4 h-4 mr-1" /> Add Level
            </.button>
            <.button
              phx-click="clear_hierarchy_levels"
              variant="outline"
              size="small"
              data-confirm="Are you sure you want to delete all hierarchy levels?"
            >
              <.icon name="hero-trash" class="w-4 h-4 mr-1" /> Clear All
            </.button>
          <% end %>
        </div>
      </div>

      <%= if @event.hierarchy_levels == [] do %>
        <%!-- Empty State --%>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
          <%!-- Quick Start Card --%>
          <div class="bg-white rounded-lg border-2 border-blue-200 p-6 hover:border-blue-300 transition-colors">
            <div class="flex items-center justify-center w-12 h-12 rounded-lg bg-blue-100 mb-4">
              <.icon name="hero-sparkles" class="w-6 h-6 text-blue-600" />
            </div>
            <h3 class="text-lg font-semibold text-gray-900 mb-2">Quick Start</h3>
            <p class="text-sm text-gray-600 mb-4">
              Use the standard gymnastics hierarchy structure with 5 levels: Gym, Session, Group, Apparatus, and Competitor.
            </p>
            <.button phx-click="generate_standard_hierarchy" variant="primary" class="w-full">
              <.icon name="hero-sparkles" class="w-4 h-4 mr-2" /> Generate Standard Hierarchy
            </.button>
          </div>

          <%!-- Custom Build Card --%>
          <div class="bg-white rounded-lg border-2 border-gray-200 p-6 hover:border-gray-300 transition-colors">
            <div class="flex items-center justify-center w-12 h-12 rounded-lg bg-gray-100 mb-4">
              <.icon name="hero-wrench-screwdriver" class="w-6 h-6 text-gray-600" />
            </div>
            <h3 class="text-lg font-semibold text-gray-900 mb-2">Custom Build</h3>
            <p class="text-sm text-gray-600 mb-4">
              Create a custom hierarchy structure tailored to your specific event needs.
            </p>
            <.button phx-click="add_hierarchy_level" variant="outline" class="w-full">
              <.icon name="hero-plus" class="w-4 h-4 mr-2" /> Start Custom Build
            </.button>
          </div>
        </div>
      <% else %>
        <%!-- Hierarchy Levels Table --%>
        <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
          <table class="min-w-full divide-y divide-gray-200">
            <thead class="bg-gray-50">
              <tr>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider w-20">
                  Level
                </th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                  Name
                </th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                  Plural Form
                </th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider w-28">
                  Required
                </th>
                <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider w-28">
                  Photos
                </th>
                <th class="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider w-32">
                  Actions
                </th>
              </tr>
            </thead>
            <tbody class="bg-white divide-y divide-gray-200">
              <tr
                :for={level <- Enum.sort_by(@event.hierarchy_levels, & &1.level_number)}
                class="hover:bg-gray-50 transition-colors"
              >
                <td class="px-6 py-4 whitespace-nowrap">
                  <div class="flex items-center">
                    <span class="text-sm font-bold text-gray-900 bg-gray-100 rounded-full w-8 h-8 flex items-center justify-center">
                      {level.level_number}
                    </span>
                  </div>
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-gray-900">
                  {level.level_name}
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-sm text-gray-600">
                  {level.level_name_plural}
                </td>
                <td class="px-6 py-4 whitespace-nowrap">
                  <.badge color={if level.is_required, do: "success", else: "neutral"}>
                    {if level.is_required, do: "Yes", else: "No"}
                  </.badge>
                </td>
                <td class="px-6 py-4 whitespace-nowrap">
                  <.badge color={if level.allow_photos, do: "info", else: "neutral"}>
                    {if level.allow_photos, do: "Yes", else: "No"}
                  </.badge>
                </td>
                <td class="px-6 py-4 whitespace-nowrap text-right text-sm">
                  <div class="flex items-center justify-end gap-2">
                    <.button
                      phx-click="edit_hierarchy_level"
                      phx-value-id={level.id}
                      variant="outline"
                      size="small"
                    >
                      <.icon name="hero-pencil" class="w-3 h-3" />
                    </.button>
                    <.button
                      phx-click="delete_hierarchy_level"
                      phx-value-id={level.id}
                      variant="outline"
                      size="small"
                      data-confirm="Are you sure you want to delete this level?"
                    >
                      <.icon name="hero-trash" class="w-3 h-3" />
                    </.button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <%!-- Info Box --%>
        <div class="mt-6 p-4 bg-blue-50 rounded-lg border border-blue-100">
          <div class="flex items-start">
            <.icon
              name="hero-information-circle"
              class="w-5 h-5 text-blue-600 mr-3 mt-0.5 flex-shrink-0"
            />
            <div>
              <h4 class="text-sm font-semibold text-blue-900 mb-1">Next Step</h4>
              <p class="text-sm text-blue-800">
                Once you've configured your hierarchy levels, go to the
                <strong>Hierarchy Nodes</strong>
                tab to use the Structure Builder and generate your folder structure.
              </p>
            </div>
          </div>
        </div>
      <% end %>
    </div>
    """
  end

  defp render_nodes(assigns) do
    ~H"""
    <div class="max-w-4xl">
      <div class="flex items-center justify-between mb-6">
        <div>
          <h2 class="text-2xl font-bold text-gray-900">Hierarchy Nodes</h2>
          <p class="text-sm text-gray-500 mt-1">
            Generated folder structure for your event
          </p>
        </div>
        <%= if @event.hierarchy_levels != [] do %>
          <.button phx-click="start_builder" variant="primary">
            <.icon name="hero-plus" class="w-4 h-4 mr-2" /> Build Structure
          </.button>
        <% end %>
      </div>

      <%= if @event.hierarchy_levels == [] do %>
        <div class="text-center py-16 bg-white rounded-lg border-2 border-dashed border-gray-300">
          <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-yellow-100 mb-4">
            <.icon name="hero-exclamation-triangle" class="w-8 h-8 text-yellow-600" />
          </div>
          <h3 class="text-lg font-semibold text-gray-900 mb-2">Define hierarchy first</h3>
          <p class="text-gray-500 mb-6 max-w-md mx-auto">
            Please define your hierarchy levels in the "Hierarchy Levels" tab before generating nodes
          </p>
          <.button phx-click="select_view" phx-value-view="structure" variant="outline">
            <.icon name="hero-arrow-right" class="w-4 h-4 mr-2" /> Go to Hierarchy Levels
          </.button>
        </div>
      <% else %>
        <div class="text-center py-16 bg-white rounded-lg border-2 border-dashed border-gray-300">
          <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-blue-100 mb-4">
            <.icon name="hero-folder" class="w-8 h-8 text-blue-600" />
          </div>
          <h3 class="text-lg font-semibold text-gray-900 mb-2">Ready to build</h3>
          <p class="text-gray-500 mb-6 max-w-md mx-auto">
            Use the Structure Builder wizard to automatically generate your complete folder hierarchy
          </p>
          <.button phx-click="start_builder" variant="primary" size="large">
            <.icon name="hero-rocket-launch" class="w-5 h-5 mr-2" /> Start Structure Builder
          </.button>
        </div>
      <% end %>
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
    event = Ash.get!(PhotoFinish.Events.Event, id) |> Ash.load!(:hierarchy_levels)

    {:ok,
     socket
     |> assign(:page_title, event.name)
     |> assign(:show_builder, false)
     |> assign(:current_view, "overview")
     |> assign(:editing_level, nil)
     |> assign(:level_form, nil)
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

  @impl true
  def handle_event("generate_standard_hierarchy", _params, socket) do
    event_id = socket.assigns.event.id

    standard_levels = [
      %{
        level_number: 1,
        level_name: "Gym",
        level_name_plural: "Gyms",
        is_required: true,
        allow_photos: false,
        event_id: event_id
      },
      %{
        level_number: 2,
        level_name: "Session",
        level_name_plural: "Sessions",
        is_required: true,
        allow_photos: false,
        event_id: event_id
      },
      %{
        level_number: 3,
        level_name: "Group",
        level_name_plural: "Groups",
        is_required: true,
        allow_photos: false,
        event_id: event_id
      },
      %{
        level_number: 4,
        level_name: "Apparatus",
        level_name_plural: "Apparatuses",
        is_required: true,
        allow_photos: false,
        event_id: event_id
      },
      %{
        level_number: 5,
        level_name: "Competitor",
        level_name_plural: "Competitors",
        is_required: true,
        allow_photos: true,
        event_id: event_id
      }
    ]

    for attrs <- standard_levels do
      Ash.create!(HierarchyLevel, attrs)
    end

    # Reload event
    event = Ash.get!(PhotoFinish.Events.Event, event_id) |> Ash.load!(:hierarchy_levels)

    {:noreply,
     socket
     |> put_flash(:info, "Standard hierarchy generated successfully.")
     |> assign(:event, event)}
  end

  @impl true
  def handle_event("add_hierarchy_level", _params, socket) do
    next_level_number = length(socket.assigns.event.hierarchy_levels) + 1

    level_form =
      to_form(%{
        "level_name" => "",
        "level_name_plural" => "",
        "is_required" => true,
        "allow_photos" => false
      })

    {:noreply,
     socket
     |> assign(:editing_level, %{level_number: next_level_number})
     |> assign(:level_form, level_form)}
  end

  @impl true
  def handle_event("edit_hierarchy_level", %{"id" => id}, socket) do
    level = Enum.find(socket.assigns.event.hierarchy_levels, &(&1.id == id))

    level_form =
      to_form(%{
        "level_name" => level.level_name,
        "level_name_plural" => level.level_name_plural,
        "is_required" => level.is_required,
        "allow_photos" => level.allow_photos
      })

    {:noreply,
     socket
     |> assign(:editing_level, %{id: level.id, level_number: level.level_number})
     |> assign(:level_form, level_form)}
  end

  @impl true
  def handle_event("delete_hierarchy_level", %{"id" => id}, socket) do
    level = Enum.find(socket.assigns.event.hierarchy_levels, &(&1.id == id))
    Ash.destroy!(level)

    # Reload event
    event =
      Ash.get!(PhotoFinish.Events.Event, socket.assigns.event.id)
      |> Ash.load!(:hierarchy_levels)

    {:noreply,
     socket
     |> put_flash(:info, "Hierarchy level deleted successfully.")
     |> assign(:event, event)}
  end

  @impl true
  def handle_event("clear_hierarchy_levels", _params, socket) do
    for level <- socket.assigns.event.hierarchy_levels do
      Ash.destroy!(level)
    end

    # Reload event
    event =
      Ash.get!(PhotoFinish.Events.Event, socket.assigns.event.id)
      |> Ash.load!(:hierarchy_levels)

    {:noreply,
     socket
     |> put_flash(:info, "All hierarchy levels cleared.")
     |> assign(:event, event)}
  end

  @impl true
  def handle_event(
        "save_hierarchy_level",
        %{"level_name" => level_name, "level_name_plural" => level_name_plural} = params,
        socket
      ) do
    is_required = Map.get(params, "is_required") == "true"
    allow_photos = Map.get(params, "allow_photos") == "true"

    attrs = %{
      level_number: socket.assigns.editing_level.level_number,
      level_name: level_name,
      level_name_plural: level_name_plural,
      is_required: is_required,
      allow_photos: allow_photos,
      event_id: socket.assigns.event.id
    }

    case socket.assigns.editing_level[:id] do
      nil ->
        # Create new level
        Ash.create!(HierarchyLevel, attrs)

      id ->
        # Update existing level
        level = Enum.find(socket.assigns.event.hierarchy_levels, &(&1.id == id))
        Ash.update!(level, attrs)
    end

    # Reload event
    event =
      Ash.get!(PhotoFinish.Events.Event, socket.assigns.event.id)
      |> Ash.load!(:hierarchy_levels)

    {:noreply,
     socket
     |> put_flash(:info, "Hierarchy level saved successfully.")
     |> assign(:event, event)
     |> assign(:editing_level, nil)
     |> assign(:level_form, nil)}
  end

  @impl true
  def handle_event("cancel_edit_level", _params, socket) do
    {:noreply,
     socket
     |> assign(:editing_level, nil)
     |> assign(:level_form, nil)}
  end

  @impl true
  def handle_event("stop_propagation", _params, socket) do
    {:noreply, socket}
  end

  @impl true
  def handle_event("start_builder", _params, socket) do
    {:noreply, assign(socket, :show_builder, true)}
  end

  @impl true
  def handle_event("close_builder_modal", _params, socket) do
    {:noreply, assign(socket, :show_builder, false)}
  end

  @impl true
  def handle_info(:close_builder, socket) do
    {:noreply, assign(socket, :show_builder, false)}
  end

  @impl true
  def handle_info({:start_generation, level_configs}, socket) do
    require Logger
    event = socket.assigns.event

    Logger.info("Starting hierarchy generation for event #{event.id}")
    Logger.info("Level configs: #{inspect(level_configs)}")
    Logger.info("Storage directory: #{inspect(event.storage_directory)}")

    # Check if storage_directory is set
    if !event.storage_directory do
      Logger.warning("Cannot create folders: No storage directory set for event #{event.id}")

      {:noreply,
       socket
       |> put_flash(
         :error,
         "Cannot create folders: No storage directory set for this event. Please set a storage directory in the event settings."
       )}
    else
      case PhotoFinish.Events.HierarchyGenerator.generate_hierarchy(event, level_configs, true) do
        {:ok, stats} ->
          Logger.info("Successfully generated hierarchy for event #{event.id}: #{inspect(stats)}")

          # Reload event with updated data
          event =
            Ash.get!(PhotoFinish.Events.Event, event.id)
            |> Ash.load!([:hierarchy_levels, :hierarchy_nodes])

          folder_msg =
            if stats.folders_created > 0 do
              "and created #{stats.folders_created} folders at #{event.storage_directory}"
            else
              "but no folders were created (check that storage directory exists)"
            end

          {:noreply,
           socket
           |> put_flash(
             :info,
             "Successfully generated #{stats.nodes_created} hierarchy nodes #{folder_msg}!"
           )
           |> assign(:event, event)
           |> assign(:show_builder, false)}

        {:error, reason} ->
          Logger.error("Failed to generate hierarchy for event #{event.id}: #{inspect(reason)}")

          {:noreply,
           socket
           |> put_flash(:error, "Failed to generate hierarchy: #{reason}")}
      end
    end
  end

  @impl true
  def handle_info({:builder_finished, msg}, socket) do
    {:noreply,
     socket
     |> put_flash(:info, msg)
     |> assign(:show_builder, false)}
  end
end
