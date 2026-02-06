defmodule PhotoFinishWeb.Admin.ProductLive.Index do
  use PhotoFinishWeb, :live_view

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.admin flash={@flash}>
      <%!-- Top Toolbar --%>
      <div class="bg-white border-b border-gray-200 px-6 py-4">
        <div class="flex items-center justify-between">
          <div>
            <h1 class="text-2xl font-bold text-gray-900">Products</h1>
            <p class="text-sm text-gray-500 mt-1">Manage your product catalog templates</p>
          </div>
          <div class="flex items-center gap-3">
            <.button_link navigate={~p"/admin/products/new"} size="large">
              <.icon name="hero-plus" class="w-5 h-5 mr-2" /> New Product
            </.button_link>
          </div>
        </div>
      </div>

      <%!-- Main Content --%>
      <div class="p-6">
        <%= if @products_count == 0 do %>
          <%!-- Empty State --%>
          <div class="text-center py-12">
            <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-gray-100 mb-4">
              <.icon name="hero-shopping-bag" class="w-8 h-8 text-gray-400" />
            </div>
            <h3 class="text-lg font-semibold text-gray-900 mb-2">No products yet</h3>
            <p class="text-gray-500 mb-6">Get started by creating your first product template</p>
            <.button_link navigate={~p"/admin/products/new"} variant="primary">
              <.icon name="hero-plus" class="w-4 h-4 mr-2" /> Create Your First Product
            </.button_link>
          </div>
        <% else %>
          <%!-- Products Table --%>
          <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
            <table class="min-w-full divide-y divide-gray-200">
              <thead class="bg-gray-50">
                <tr>
                  <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Product Name
                  </th>
                  <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Type
                  </th>
                  <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Size
                  </th>
                  <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Default Price
                  </th>
                  <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Status
                  </th>
                  <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Order
                  </th>
                  <th class="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider">
                    Actions
                  </th>
                </tr>
              </thead>
              <tbody class="bg-white divide-y divide-gray-200" id="products" phx-update="stream">
                <tr
                  :for={{id, product} <- @streams.products}
                  id={id}
                  class="hover:bg-gray-50 transition-colors"
                >
                  <td class="px-6 py-4 whitespace-nowrap">
                    <div class="text-sm font-medium text-gray-900">{product.product_name}</div>
                  </td>
                  <td class="px-6 py-4 whitespace-nowrap">
                    <.badge color={type_color(product.product_type)}>
                      {Phoenix.Naming.humanize(product.product_type)}
                    </.badge>
                  </td>
                  <td class="px-6 py-4 whitespace-nowrap">
                    <span class="text-sm text-gray-600">{product.product_size || "-"}</span>
                  </td>
                  <td class="px-6 py-4 whitespace-nowrap">
                    <span class="text-sm font-medium text-gray-900">
                      ${format_price(product.default_price_cents)}
                    </span>
                  </td>
                  <td class="px-6 py-4 whitespace-nowrap">
                    <.badge color={if product.is_active, do: "success", else: "natural"}>
                      {if product.is_active, do: "Active", else: "Inactive"}
                    </.badge>
                  </td>
                  <td class="px-6 py-4 whitespace-nowrap">
                    <span class="text-sm text-gray-500">{product.display_order}</span>
                  </td>
                  <td class="px-6 py-4 whitespace-nowrap text-right">
                    <div class="flex items-center justify-end gap-2">
                      <.button
                        type="button"
                        size="small"
                        variant="outline"
                        phx-click={JS.navigate(~p"/admin/products/#{product.id}/edit")}
                      >
                        <.icon name="hero-pencil" class="w-4 h-4 mr-1" /> Edit
                      </.button>
                      <.button
                        type="button"
                        size="small"
                        variant="outline"
                        phx-click={JS.push("delete", value: %{id: product.id}) |> hide("##{id}")}
                        data-confirm="Are you sure you want to delete this product?"
                      >
                        <.icon name="hero-trash" class="w-4 h-4 mr-1" /> Delete
                      </.button>
                    </div>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        <% end %>
      </div>
    </Layouts.admin>
    """
  end

  @impl true
  def mount(_params, _session, socket) do
    products = Ash.read!(PhotoFinish.Orders.ProductTemplate)

    {:ok,
     socket
     |> assign(:page_title, "Products")
     |> assign(:products_count, length(products))
     |> stream(:products, products)}
  end

  @impl true
  def handle_event("delete", %{"id" => id}, socket) do
    product = Ash.get!(PhotoFinish.Orders.ProductTemplate, id)
    Ash.destroy!(product)

    {:noreply,
     socket
     |> stream_delete(:products, product)
     |> assign(:products_count, socket.assigns.products_count - 1)
     |> put_flash(:info, "Product deleted successfully")}
  end

  defp format_price(cents) when is_integer(cents) do
    dollars = div(cents, 100)
    remainder = rem(cents, 100)
    "#{dollars}.#{String.pad_leading(Integer.to_string(remainder), 2, "0")}"
  end

  defp format_price(_), do: "0.00"

  defp type_color(:usb), do: "info"
  defp type_color(:print), do: "primary"
  defp type_color(:collage), do: "warning"
  defp type_color(:custom_photo), do: "success"
  defp type_color(:accessory), do: "secondary"
  defp type_color(_), do: "natural"
end
