defmodule PhotoFinishWeb.Admin.ProductLive.Form do
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
              <.button_link navigate={~p"/admin/products"} variant="outline" size="small">
                <.icon name="hero-arrow-left" class="w-4 h-4" />
              </.button_link>
              <div>
                <h1 class="text-xl font-bold text-gray-900">{@page_title}</h1>
                <p class="text-xs text-gray-500">Configure product template details</p>
              </div>
            </div>
            <div class="flex items-center gap-2">
              <.button_link navigate={~p"/admin/products"} size="small" variant="outline">
                Cancel
              </.button_link>
              <.button
                form="product-template-form"
                type="submit"
                phx-disable-with="Saving..."
                size="small"
              >
                <.icon name="hero-check" class="w-4 h-4 mr-1" /> Save Product
              </.button>
            </div>
          </div>
        </div>

        <%!-- Main Content Area --%>
        <div class="flex-1 overflow-y-auto">
          <div class="max-w-3xl mx-auto p-8">
            <.form
              for={@form}
              id="product-template-form"
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
                      field={@form[:product_name]}
                      type="text"
                      label="Product Name"
                      placeholder="e.g., 5x7 Print"
                      required
                    />

                    <.input
                      field={@form[:product_type]}
                      type="select"
                      label="Product Type"
                      options={product_type_options()}
                      required
                    />
                  </div>

                  <.input
                    field={@form[:product_size]}
                    type="text"
                    label="Size"
                    placeholder="e.g., 5x7, 8x10, 16GB"
                  />
                </div>
              </div>

              <%!-- Pricing & Display Section --%>
              <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
                <div class="px-6 py-4 border-b border-gray-200 bg-gray-50">
                  <h3 class="text-lg font-semibold text-gray-900 flex items-center">
                    <.icon name="hero-currency-dollar" class="w-5 h-5 mr-2 text-gray-400" />
                    Pricing & Display
                  </h3>
                </div>
                <div class="p-6 space-y-6">
                  <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                    <div>
                      <.input
                        field={@form[:default_price_cents]}
                        type="number"
                        label="Default Price (cents)"
                        placeholder="1500"
                        min="0"
                        required
                      />
                      <p class="mt-2 text-xs text-gray-500 flex items-start">
                        <.icon
                          name="hero-information-circle"
                          class="w-4 h-4 mr-1 mt-0.5 flex-shrink-0"
                        />
                        <span>Enter price in cents (e.g., 1500 = $15.00)</span>
                      </p>
                    </div>

                    <.input
                      field={@form[:display_order]}
                      type="number"
                      label="Display Order"
                      placeholder="0"
                      min="0"
                    />
                  </div>

                  <.input
                    field={@form[:is_active]}
                    type="checkbox"
                    label="Active"
                  />
                  <p class="text-xs text-gray-500 -mt-4">
                    Inactive products will not be available for new events
                  </p>
                </div>
              </div>
            </.form>
          </div>
        </div>
      </div>
    </Layouts.admin>
    """
  end

  @impl true
  def mount(params, _session, socket) do
    product =
      case params["id"] do
        nil -> nil
        id -> Ash.get!(PhotoFinish.Orders.ProductTemplate, id)
      end

    is_edit = not is_nil(product)
    action = if is_edit, do: "Edit", else: "New"
    page_title = action <> " " <> "Product"

    {:ok,
     socket
     |> assign(product: product)
     |> assign(:is_edit, is_edit)
     |> assign(:page_title, page_title)
     |> assign_form()}
  end

  @impl true
  def handle_event("validate", %{"product_template" => params}, socket) do
    {:noreply, assign(socket, form: AshPhoenix.Form.validate(socket.assigns.form, params))}
  end

  @impl true
  def handle_event("save", %{"product_template" => params}, socket) do
    case AshPhoenix.Form.submit(socket.assigns.form, params: params) do
      {:ok, _product} ->
        action_word = if socket.assigns.is_edit, do: "updated", else: "created"

        {:noreply,
         socket
         |> put_flash(:info, "Product #{action_word} successfully")
         |> push_navigate(to: ~p"/admin/products")}

      {:error, form} ->
        {:noreply, assign(socket, form: form)}
    end
  end

  defp assign_form(%{assigns: %{product: product}} = socket) do
    form =
      if product do
        AshPhoenix.Form.for_update(product, :update, as: "product_template")
      else
        AshPhoenix.Form.for_create(PhotoFinish.Orders.ProductTemplate, :create,
          as: "product_template"
        )
      end

    assign(socket, form: to_form(form))
  end

  defp product_type_options do
    Ash.Resource.Info.attribute(PhotoFinish.Orders.ProductTemplate, :product_type).constraints[
      :one_of
    ]
    |> Enum.map(fn type -> {Phoenix.Naming.humanize(type), type} end)
  end
end
