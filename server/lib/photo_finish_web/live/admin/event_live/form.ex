defmodule PhotoFinishWeb.Admin.EventLive.Form do
  use PhotoFinishWeb, :live_view

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.admin flash={@flash}>
      <div class="h-screen flex flex-col bg-gray-50">
        <%!-- Top Toolbar --%>
        <div class="bg-white border-b border-gray-200 px-6 py-3 flex-shrink-0">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-4">
              <.button_link navigate={return_path(@return_to, @event)} variant="outline" size="small">
                <.icon name="hero-arrow-left" class="w-4 h-4" />
              </.button_link>
              <div>
                <h1 class="text-xl font-bold text-gray-900">{@page_title}</h1>
                <p class="text-xs text-gray-500">Configure your event details</p>
              </div>
            </div>
            <div class="flex items-center gap-2">
              <.button_link navigate={return_path(@return_to, @event)} size="small" variant="outline">
                Cancel
              </.button_link>
              <.button form="event-form" type="submit" phx-disable-with="Saving..." variant="primary" size="small">
                <.icon name="hero-check" class="w-4 h-4 mr-1" />
                Save Event
              </.button>
            </div>
          </div>
        </div>

        <%!-- Main Content Area --%>
        <div class="flex-1 flex overflow-hidden">
          <%!-- Form Content --%>
          <div class="flex-1 overflow-y-auto">
            <div class="max-w-4xl mx-auto p-8">
              <.form
                for={@form}
                id="event-form"
                phx-change="validate"
                phx-submit="save"
                class="space-y-8"
              >
                <%!-- Basic Information Section --%>
                <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
                  <div class="px-6 py-4 border-b border-gray-200 bg-gray-50">
                    <h3 class="text-lg font-semibold text-gray-900 flex items-center">
                      <.icon name="hero-information-circle" class="w-5 h-5 mr-2 text-gray-400" />
                      Basic Information
                    </h3>
                  </div>
                  <div class="p-6 space-y-6">
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                      <.input
                        field={@form[:name]}
                        type="text"
                        label="Event Name"
                        placeholder="e.g., Spring Valley Gymnastics Meet 2025"
                        required
                      />

                      <.input
                        field={@form[:slug]}
                        type="text"
                        label="URL Slug"
                        placeholder="e.g., svgm-2025"
                      />
                    </div>

                    <.input
                      field={@form[:description]}
                      type="textarea"
                      label="Description"
                      placeholder="Brief description of the event"
                      rows="4"
                    />
                  </div>
                </div>

                <%!-- Event Schedule Section --%>
                <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
                  <div class="px-6 py-4 border-b border-gray-200 bg-gray-50">
                    <h3 class="text-lg font-semibold text-gray-900 flex items-center">
                      <.icon name="hero-calendar" class="w-5 h-5 mr-2 text-gray-400" />
                      Event Schedule
                    </h3>
                  </div>
                  <div class="p-6">
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                      <div>
                        <.input
                          field={@form[:starts_at]}
                          type="datetime-local"
                          label="Start Date & Time"
                        />
                        <p class="mt-2 text-xs text-gray-500">
                          When does the event begin?
                        </p>
                      </div>

                      <div>
                        <.input
                          field={@form[:ends_at]}
                          type="datetime-local"
                          label="End Date & Time"
                        />
                        <p class="mt-2 text-xs text-gray-500">
                          When does the event conclude?
                        </p>
                      </div>
                    </div>
                  </div>
                </div>

                <%!-- Technical Configuration Section --%>
                <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
                  <div class="px-6 py-4 border-b border-gray-200 bg-gray-50">
                    <h3 class="text-lg font-semibold text-gray-900 flex items-center">
                      <.icon name="hero-cog-6-tooth" class="w-5 h-5 mr-2 text-gray-400" />
                      Technical Configuration
                    </h3>
                  </div>
                  <div class="p-6 space-y-6">
                    <div>
                      <.input
                        field={@form[:storage_directory]}
                        type="text"
                        label="Storage Directory"
                        placeholder="/mnt/nas/photos/events/svgm-2025"
                      />
                      <p class="mt-2 text-xs text-gray-500 flex items-start">
                        <.icon name="hero-information-circle" class="w-4 h-4 mr-1 mt-0.5 flex-shrink-0" />
                        <span>Absolute path where photo files will be stored on the server</span>
                      </p>
                    </div>

                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                      <.input
                        field={@form[:status]}
                        type="select"
                        label="Event Status"
                        options={
                          Ash.Resource.Info.attribute(PhotoFinish.Events.Event, :status).constraints[
                            :one_of
                          ]
                        }
                      />
                    </div>
                  </div>
                </div>

                <%!-- Order & Pricing Section --%>
                <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
                  <div class="px-6 py-4 border-b border-gray-200 bg-gray-50">
                    <h3 class="text-lg font-semibold text-gray-900 flex items-center">
                      <.icon name="hero-shopping-cart" class="w-5 h-5 mr-2 text-gray-400" />
                      Order & Pricing
                    </h3>
                  </div>
                  <div class="p-6">
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                      <div>
                        <.input
                          field={@form[:order_code]}
                          type="text"
                          label="Order Code Prefix"
                          placeholder="STV"
                          maxlength="3"
                        />
                        <p class="mt-2 text-xs text-gray-500 flex items-start">
                          <.icon name="hero-information-circle" class="w-4 h-4 mr-1 mt-0.5 flex-shrink-0" />
                          <span>3-letter code for order numbers (e.g., STV-0123)</span>
                        </p>
                      </div>

                      <div>
                        <.input
                          field={@form[:tax_rate_basis_points]}
                          type="number"
                          label="Tax Rate (basis points)"
                          placeholder="850"
                        />
                        <p class="mt-2 text-xs text-gray-500 flex items-start">
                          <.icon name="hero-information-circle" class="w-4 h-4 mr-1 mt-0.5 flex-shrink-0" />
                          <span>Enter 850 for 8.5% tax rate</span>
                        </p>
                      </div>
                    </div>
                  </div>
                </div>
              </.form>
            </div>
          </div>

          <%!-- Right Sidebar - Form Preview --%>
          <div class="w-80 bg-white border-l border-gray-200 overflow-y-auto flex-shrink-0">
            <div class="p-6">
              <h3 class="text-sm font-semibold text-gray-900 mb-4">Preview</h3>

              <%!-- Event Card Preview --%>
              <div class="border border-gray-200 rounded-lg overflow-hidden">
                <div class="p-4 border-b border-gray-100">
                  <div class="flex items-start justify-between mb-2">
                    <h4 class="font-semibold text-gray-900 text-sm">
                      {Phoenix.HTML.Form.input_value(@form, :name) || "Event Name"}
                    </h4>
                    <.badge color="success">
                      {Phoenix.HTML.Form.input_value(@form, :status) || :active}
                    </.badge>
                  </div>
                  <%= if Phoenix.HTML.Form.input_value(@form, :description) do %>
                    <p class="text-xs text-gray-500 line-clamp-2">
                      {Phoenix.HTML.Form.input_value(@form, :description)}
                    </p>
                  <% end %>
                </div>

                <div class="p-4 space-y-3 text-xs">
                  <%= if Phoenix.HTML.Form.input_value(@form, :starts_at) do %>
                    <div class="flex items-center text-gray-600">
                      <.icon name="hero-calendar" class="w-3 h-3 mr-2 text-gray-400" />
                      <span>
                        {format_datetime(Phoenix.HTML.Form.input_value(@form, :starts_at))}
                      </span>
                    </div>
                  <% end %>

                  <%= if Phoenix.HTML.Form.input_value(@form, :order_code) do %>
                    <div class="flex items-center text-gray-600">
                      <.icon name="hero-ticket" class="w-3 h-3 mr-2 text-gray-400" />
                      <span>Code: {Phoenix.HTML.Form.input_value(@form, :order_code)}</span>
                    </div>
                  <% end %>

                  <%= if Phoenix.HTML.Form.input_value(@form, :slug) do %>
                    <div class="flex items-center text-gray-600">
                      <.icon name="hero-link" class="w-3 h-3 mr-2 text-gray-400" />
                      <span class="font-mono">{Phoenix.HTML.Form.input_value(@form, :slug)}</span>
                    </div>
                  <% end %>
                </div>
              </div>

              <%!-- Quick Stats --%>
              <div class="mt-6 space-y-3">
                <div class="flex items-center justify-between text-xs">
                  <span class="text-gray-500">Status</span>
                  <span class="font-medium capitalize">
                    {Phoenix.HTML.Form.input_value(@form, :status) || "active"}
                  </span>
                </div>

                <%= if Phoenix.HTML.Form.input_value(@form, :tax_rate_basis_points) do %>
                  <div class="flex items-center justify-between text-xs">
                    <span class="text-gray-500">Tax Rate</span>
                    <span class="font-medium">
                      {Phoenix.HTML.Form.input_value(@form, :tax_rate_basis_points) / 100}%
                    </span>
                  </div>
                <% end %>
              </div>

              <%!-- Tips --%>
              <div class="mt-6 p-4 bg-blue-50 rounded-lg border border-blue-100">
                <h4 class="text-xs font-semibold text-blue-900 mb-2 flex items-center">
                  <.icon name="hero-light-bulb" class="w-4 h-4 mr-1" />
                  Tips
                </h4>
                <ul class="text-xs text-blue-800 space-y-1">
                  <li>• Use a descriptive event name</li>
                  <li>• Keep slugs short and URL-friendly</li>
                  <li>• Order codes should be 3 letters</li>
                  <li>• Tax rates are in basis points (850 = 8.5%)</li>
                </ul>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Layouts.admin>
    """
  end

  defp format_datetime(nil), do: "Not set"

  defp format_datetime(datetime) when is_binary(datetime) do
    case DateTime.from_iso8601(datetime) do
      {:ok, dt, _} -> Calendar.strftime(dt, "%B %d, %Y")
      _ -> datetime
    end
  end

  defp format_datetime(%DateTime{} = datetime) do
    Calendar.strftime(datetime, "%B %d, %Y")
  end

  defp format_datetime(_), do: "Not set"

  @impl true
  def mount(params, _session, socket) do
    event =
      case params["id"] do
        nil -> nil
        id -> Ash.get!(PhotoFinish.Events.Event, id)
      end

    action = if is_nil(event), do: "New", else: "Edit"
    page_title = action <> " " <> "Event"

    {:ok,
     socket
     |> assign(:return_to, return_to(params["return_to"]))
     |> assign(event: event)
     |> assign(:page_title, page_title)
     |> assign_form()}
  end

  defp return_to("show"), do: "show"
  defp return_to(_), do: "index"

  @impl true
  def handle_event("validate", %{"event" => event_params}, socket) do
    {:noreply, assign(socket, form: AshPhoenix.Form.validate(socket.assigns.form, event_params))}
  end

  def handle_event("save", %{"event" => event_params}, socket) do
    case AshPhoenix.Form.submit(socket.assigns.form, params: event_params) do
      {:ok, event} ->
        notify_parent({:saved, event})

        socket =
          socket
          |> put_flash(:info, "Event #{socket.assigns.form.source.type}d successfully")
          |> push_navigate(to: return_path(socket.assigns.return_to, event))

        {:noreply, socket}

      {:error, form} ->
        {:noreply, assign(socket, form: form)}
    end
  end

  defp notify_parent(msg), do: send(self(), {__MODULE__, msg})

  defp assign_form(%{assigns: %{event: event}} = socket) do
    form =
      if event do
        AshPhoenix.Form.for_update(event, :update, as: "event")
      else
        AshPhoenix.Form.for_create(PhotoFinish.Events.Event, :create, as: "event")
      end

    assign(socket, form: to_form(form))
  end

  defp return_path("index", _event), do: ~p"/admin/events"
  defp return_path("show", event), do: ~p"/admin/events/#{event.id}"
end
