defmodule PhotoFinishWeb.Admin.EventLive.Components.StructureBuilder do
  use PhotoFinishWeb, :live_component

  def render(assigns) do
    ~H"""
    <div class="space-y-6">
      <div class="flex justify-between items-center">
        <div>
          <h3 class="text-lg font-bold">Hierarchy Structure Builder</h3>
          <p class="text-sm text-gray-500 mt-1">
            Configure and generate folders for your event hierarchy
          </p>
        </div>
        <.button phx-click="cancel_builder" phx-target={@myself} size="small">
          Close
        </.button>
      </div>

      <%!-- Progress Indicator --%>
      <div class="bg-gray-100 rounded-lg p-4">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <.icon name="hero-cube-transparent" class="w-5 h-5 text-gray-600" />
            <span class="text-sm font-medium text-gray-700">
              Configuring Level {@current_level_idx}: {current_level_name(assigns)}
            </span>
          </div>
          <span class="text-xs text-gray-500">
            {length(@configured_levels)} of {length(@hierarchy_levels)} levels configured
          </span>
        </div>
      </div>

      <%= if @show_review do %>
        <%!-- Review Screen --%>
        {render_review_screen(assigns)}
      <% else %>
        <%!-- Level Configuration Form --%>
        {render_level_form(assigns)}
      <% end %>
    </div>
    """
  end

  defp render_level_form(assigns) do
    ~H"""
    <.card>
      <.card_content>
        <.form
          for={@form}
          phx-change="validate"
          phx-submit="save_level"
          phx-target={@myself}
          class="space-y-6"
        >
          <div>
            <h4 class="font-semibold text-lg mb-2">
              Configure: {current_level_name(assigns)}
            </h4>
            <p class="text-sm text-gray-500">
              Define how many instances and naming pattern for this level
            </p>
          </div>

          <div class="space-y-4">
            <%!-- Display Level Name (read-only) --%>
            <div class="bg-gray-50 border border-gray-200 rounded-lg p-4">
              <div class="text-sm text-gray-500 mb-1">Level Name</div>
              <div class="font-semibold text-gray-900">{current_level_name(assigns)}</div>
            </div>

            <%!-- How Many --%>
            <.input
              field={@form[:count]}
              type="number"
              label="How many instances?"
              placeholder="e.g., 6 gyms, 12 sessions"
              min="1"
              required
            />

            <%!-- Naming Pattern --%>
            <.input
              field={@form[:naming_pattern]}
              type="select"
              label="Naming Pattern"
              options={[
                {"Numeric (1, 2, 3...)", "numeric"},
                {"Alphabetic (A, B, C...)", "alpha"},
                {"Custom Prefix + Number", "custom"}
              ]}
              required
            />

            <%= if Phoenix.HTML.Form.input_value(@form, :naming_pattern) == "custom" do %>
              <div>
                <.input
                  field={@form[:custom_prefix]}
                  type="text"
                  label="Custom Prefix"
                  placeholder="e.g., Session"
                />
                <p class="mt-1 text-sm text-gray-500">
                  Will generate: Session 1, Session 2, Session 3...
                </p>
              </div>
            <% end %>

            <%!-- A/B Rotations --%>
            <div class="flex items-start gap-3">
              <.input
                field={@form[:add_rotations]}
                type="checkbox"
                label="Add A/B rotations?"
              />
              <span class="text-sm text-gray-500">
                Creates two versions of each instance (e.g., "1A" and "1B")
              </span>
            </div>

            <%!-- Preview --%>
            <%= if Phoenix.HTML.Form.input_value(@form, :count) && String.to_integer(Phoenix.HTML.Form.input_value(@form, :count) || "0") > 0 do %>
              <div class="bg-blue-50 border border-blue-200 rounded-lg p-4">
                <div class="text-sm font-medium text-blue-900 mb-2">Preview:</div>
                <div class="text-xs font-mono text-blue-700">
                  {preview_names_from_form(assigns) |> Enum.take(5) |> Enum.join(", ")}
                  <%= if length(preview_names_from_form(assigns)) > 5 do %>
                    <span class="text-blue-500">
                      ... and {length(preview_names_from_form(assigns)) - 5} more
                    </span>
                  <% end %>
                </div>
              </div>
            <% end %>
          </div>

          <%!-- Action Buttons --%>
          <div class="pt-4 border-t space-y-3">
            <%= if length(@configured_levels) > 0 do %>
              <.button
                type="button"
                phx-click="show_review"
                phx-target={@myself}
                variant="outline"
                class="w-full"
              >
                <.icon name="hero-eye" class="w-4 h-4 mr-2" />
                Review & Generate ({length(@configured_levels)} Level{if length(@configured_levels) >
                                                                           1, do: "s"} Configured)
              </.button>
            <% end %>

            <div class="flex items-center gap-3">
              <%= if length(@configured_levels) > 0 do %>
                <.button
                  type="button"
                  phx-click="previous_step"
                  phx-target={@myself}
                  variant="outline"
                  class="flex-1"
                >
                  <.icon name="hero-arrow-left" class="w-4 h-4 mr-1" /> Previous
                </.button>
              <% end %>

              <.button type="submit" variant="primary" class="flex-1">
                Save & Continue <.icon name="hero-arrow-right" class="w-4 h-4 ml-1" />
              </.button>
            </div>
          </div>
        </.form>
      </.card_content>
    </.card>
    """
  end

  defp render_review_screen(assigns) do
    ~H"""
    <.card>
      <.card_content>
        <div class="space-y-6">
          <div>
            <h4 class="font-semibold text-lg mb-2">Review & Generate</h4>
            <p class="text-sm text-gray-500">
              Confirm the configuration before generating nodes and folders
            </p>
          </div>

          <div class="space-y-3">
            <div
              :for={{config, idx} <- Enum.with_index(@configured_levels)}
              class="border rounded-lg p-4 bg-white hover:bg-gray-50 transition-colors"
            >
              <div class="flex items-start justify-between">
                <div class="flex-1">
                  <div class="flex items-center gap-2 mb-2">
                    <span class="inline-flex items-center justify-center w-6 h-6 rounded-full bg-blue-100 text-blue-700 text-xs font-semibold">
                      {idx + 1}
                    </span>
                    <h5 class="font-semibold text-gray-900">{config.level_name}</h5>
                  </div>

                  <div class="grid grid-cols-2 gap-3 text-sm">
                    <div>
                      <span class="text-gray-500">Count:</span>
                      <span class="ml-2 font-medium text-gray-900">{config.count}</span>
                    </div>
                    <div>
                      <span class="text-gray-500">Pattern:</span>
                      <span class="ml-2 font-medium text-gray-900 capitalize">
                        {config.naming_pattern}
                      </span>
                    </div>
                    <%= if config.add_rotations do %>
                      <div class="col-span-2">
                        <.badge color="info">A/B Rotations</.badge>
                      </div>
                    <% end %>
                  </div>

                  <%!-- Preview --%>
                  <div class="mt-3 p-2 bg-gray-50 rounded text-xs font-mono text-gray-700">
                    {preview_names(config) |> Enum.take(4) |> Enum.join(", ")}
                    <%= if length(preview_names(config)) > 4 do %>
                      <span class="text-gray-500">... +{length(preview_names(config)) - 4} more</span>
                    <% end %>
                  </div>
                </div>

                <.button
                  type="button"
                  size="small"
                  variant="outline"
                  phx-click="edit_level"
                  phx-value-level={idx}
                  phx-target={@myself}
                >
                  <.icon name="hero-pencil" class="w-3 h-3" />
                </.button>
              </div>
            </div>
          </div>

          <%!-- Can Configure More Levels --%>
          <%= if @current_level_idx <= length(@hierarchy_levels) do %>
            <div class="border-2 border-dashed border-gray-300 rounded-lg p-4 text-center">
              <p class="text-sm text-gray-600 mb-3">
                You can configure {length(@hierarchy_levels) - length(@configured_levels)} more level{if length(
                                                                                                           @hierarchy_levels
                                                                                                         ) -
                                                                                                           length(
                                                                                                             @configured_levels
                                                                                                           ) >
                                                                                                           1,
                                                                                                         do:
                                                                                                           "s"}
              </p>
              <.button
                type="button"
                phx-click="continue_configuring"
                phx-target={@myself}
                variant="outline"
                size="small"
              >
                <.icon name="hero-plus" class="w-4 h-4 mr-1" />
                Configure {Enum.at(@hierarchy_levels, @current_level_idx - 1)[:level_name]}
              </.button>
            </div>
          <% end %>

          <%!-- Estimated Nodes --%>
          <div class="bg-blue-50 border border-blue-200 rounded-lg p-4">
            <div class="flex items-center gap-2 text-blue-900">
              <.icon name="hero-information-circle" class="w-5 h-5" />
              <div>
                <div class="font-medium">
                  This will create approximately {estimate_total_nodes(@configured_levels)} hierarchy nodes
                </div>
                <div class="text-sm text-blue-700 mt-1">
                  Folders will be created for levels 1-{length(@configured_levels)}
                </div>
              </div>
            </div>
          </div>

          <%!-- Action Buttons --%>
          <div class="flex items-center gap-3 pt-4 border-t">
            <.button
              type="button"
              phx-click="back_to_config"
              phx-target={@myself}
              variant="outline"
              class="flex-1"
            >
              <.icon name="hero-arrow-left" class="w-4 h-4 mr-1" /> Back to Configuration
            </.button>

            <.button
              type="button"
              phx-click="generate_structure"
              phx-target={@myself}
              variant="primary"
              size="large"
              class="flex-1"
            >
              <.icon name="hero-rocket-launch" class="w-5 h-5 mr-2" /> Generate Nodes & Folders
            </.button>
          </div>
        </div>
      </.card_content>
    </.card>
    """
  end

  def update(assigns, socket) do
    # Load hierarchy levels from the event (sorted by level_number)
    hierarchy_levels =
      assigns.event.hierarchy_levels
      |> Enum.sort_by(& &1.level_number)
      |> Enum.map(fn level ->
        %{
          id: level.id,
          level_number: level.level_number,
          level_name: level.level_name,
          allow_photos: level.allow_photos
        }
      end)

    socket =
      socket
      |> assign(assigns)
      |> assign_new(:hierarchy_levels, fn -> hierarchy_levels end)
      |> assign_new(:configured_levels, fn -> [] end)
      |> assign_new(:current_level_idx, fn -> 1 end)
      |> assign_new(:show_review, fn -> false end)
      |> assign_form()

    {:ok, socket}
  end

  defp assign_form(socket) do
    # Get the current level configuration if it exists
    current_config =
      case Enum.at(socket.assigns.configured_levels, socket.assigns.current_level_idx - 1) do
        nil ->
          # New level - use defaults
          current_level =
            Enum.at(socket.assigns.hierarchy_levels, socket.assigns.current_level_idx - 1)

          %{
            "level_name" => current_level[:level_name],
            "count" => "1",
            "naming_pattern" => "numeric",
            "add_rotations" => false,
            "custom_prefix" => ""
          }

        config ->
          # Existing configuration - load it
          %{
            "level_name" => config.level_name,
            "count" => to_string(config.count),
            "naming_pattern" => config.naming_pattern,
            "add_rotations" => config.add_rotations,
            "custom_prefix" => config[:custom_prefix] || ""
          }
      end

    assign(socket, :form, to_form(current_config, as: "level"))
  end

  def handle_event("validate", %{"level" => _level_params}, socket) do
    # Form validation happens automatically through LiveView
    {:noreply, socket}
  end

  def handle_event("save_level", %{"level" => level_params}, socket) do
    current_level = Enum.at(socket.assigns.hierarchy_levels, socket.assigns.current_level_idx - 1)

    # Save current level config
    config = %{
      level_name: current_level[:level_name],
      count: String.to_integer(level_params["count"]),
      naming_pattern: level_params["naming_pattern"],
      add_rotations: level_params["add_rotations"] == "true",
      custom_prefix: level_params["custom_prefix"] || ""
    }

    # Update or append config
    configured_levels =
      if socket.assigns.current_level_idx <= length(socket.assigns.configured_levels) do
        List.replace_at(
          socket.assigns.configured_levels,
          socket.assigns.current_level_idx - 1,
          config
        )
      else
        socket.assigns.configured_levels ++ [config]
      end

    # Move to next level if available
    next_level_idx = socket.assigns.current_level_idx + 1

    socket =
      socket
      |> assign(:configured_levels, configured_levels)
      |> assign(:current_level_idx, next_level_idx)
      |> assign_form()

    {:noreply, socket}
  end

  def handle_event("previous_step", _params, socket) do
    socket =
      socket
      |> assign(:current_level_idx, max(1, socket.assigns.current_level_idx - 1))
      |> assign(:show_review, false)
      |> assign_form()

    {:noreply, socket}
  end

  def handle_event("show_review", _params, socket) do
    {:noreply, assign(socket, :show_review, true)}
  end

  def handle_event("back_to_config", _params, socket) do
    {:noreply, assign(socket, :show_review, false)}
  end

  def handle_event("continue_configuring", _params, socket) do
    {:noreply, assign(socket, :show_review, false)}
  end

  def handle_event("edit_level", %{"level" => level_str}, socket) do
    level_idx = String.to_integer(level_str) + 1

    socket =
      socket
      |> assign(:current_level_idx, level_idx)
      |> assign(:show_review, false)
      |> assign_form()

    {:noreply, socket}
  end

  def handle_event("generate_structure", _params, socket) do
    # Send the configured levels to the parent LiveView for generation
    send(self(), {:start_generation, socket.assigns.configured_levels})
    {:noreply, socket}
  end

  def handle_event("cancel_builder", _, socket) do
    send(self(), :close_builder)
    {:noreply, socket}
  end

  # Helper functions

  defp current_level_name(assigns) do
    case Enum.at(assigns.hierarchy_levels, assigns.current_level_idx - 1) do
      nil -> "Unknown Level"
      level -> level[:level_name]
    end
  end

  defp preview_names(config) do
    generate_names(
      config.count,
      config.naming_pattern,
      config.custom_prefix,
      config.add_rotations
    )
  end

  defp preview_names_from_form(assigns) do
    count =
      case Phoenix.HTML.Form.input_value(assigns.form, :count) do
        nil -> 0
        "" -> 0
        val -> String.to_integer(val)
      end

    pattern = Phoenix.HTML.Form.input_value(assigns.form, :naming_pattern) || "numeric"
    prefix = Phoenix.HTML.Form.input_value(assigns.form, :custom_prefix) || ""
    add_rotations = Phoenix.HTML.Form.input_value(assigns.form, :add_rotations) == "true"

    generate_names(count, pattern, prefix, add_rotations)
  end

  defp generate_names(count, pattern, prefix, add_rotations) do
    base_names =
      case pattern do
        "numeric" -> Enum.map(1..count, &to_string/1)
        "alpha" -> Enum.map(1..count, &int_to_alpha/1)
        "custom" -> Enum.map(1..count, &"#{prefix} #{&1}")
        _ -> Enum.map(1..count, &to_string/1)
      end

    if add_rotations do
      Enum.flat_map(base_names, fn name -> ["#{name}A", "#{name}B"] end)
    else
      base_names
    end
  end

  defp int_to_alpha(n) when n > 0 and n <= 26 do
    <<n + 64>>
  end

  defp int_to_alpha(n) when n > 26 do
    # For numbers > 26, use AA, AB, etc.
    first = div(n - 1, 26)
    second = rem(n - 1, 26) + 1
    "#{int_to_alpha(first)}#{int_to_alpha(second)}"
  end

  defp estimate_total_nodes(configs) do
    Enum.reduce(configs, 1, fn config, acc ->
      multiplier = if config.add_rotations, do: config.count * 2, else: config.count
      acc * multiplier
    end)
  end
end
