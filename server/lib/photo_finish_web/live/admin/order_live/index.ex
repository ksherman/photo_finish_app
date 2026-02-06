defmodule PhotoFinishWeb.Admin.OrderLive.Index do
  use PhotoFinishWeb, :live_view

  require Ash.Query

  alias PhotoFinish.Orders.Order

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.admin flash={@flash}>
      <div class="min-h-screen bg-gray-50">
        <%!-- Header --%>
        <div class="bg-white border-b border-gray-200 px-6 py-4">
          <div class="max-w-5xl mx-auto">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-4">
                <.button_link
                  navigate={~p"/admin/events/#{@event}"}
                  variant="outline"
                  color="natural"
                  size="small"
                >
                  <.icon name="hero-arrow-left" class="w-4 h-4" />
                </.button_link>
                <div>
                  <h1 class="text-xl font-bold text-gray-900">Orders</h1>
                  <p class="text-sm text-gray-500">{@event.name} &mdash; {@order_count} orders</p>
                </div>
              </div>
            </div>
          </div>
        </div>

        <%!-- Main Content --%>
        <div class="max-w-5xl mx-auto py-8 px-6 space-y-6">
          <%!-- Search --%>
          <div class="bg-white rounded-lg border border-gray-200 p-4">
            <form phx-change="search" phx-submit="search">
              <div class="flex items-center gap-3">
                <.icon name="hero-magnifying-glass" class="w-5 h-5 text-gray-400" />
                <input
                  type="text"
                  name="query"
                  value={@search_query}
                  placeholder="Search by order number or customer name..."
                  phx-debounce="300"
                  class="flex-1 border-0 bg-transparent text-sm text-gray-900 placeholder-gray-400 focus:ring-0 focus:outline-none"
                />
                <%= if @search_query != "" do %>
                  <button
                    type="button"
                    phx-click="clear_search"
                    class="text-gray-400 hover:text-gray-600"
                  >
                    <.icon name="hero-x-mark" class="w-4 h-4" />
                  </button>
                <% end %>
              </div>
            </form>
          </div>

          <%!-- Orders Table --%>
          <%= if @order_count == 0 && @search_query == "" do %>
            <div class="text-center py-12">
              <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-gray-100 mb-4">
                <.icon name="hero-shopping-bag" class="w-8 h-8 text-gray-400" />
              </div>
              <h3 class="text-lg font-semibold text-gray-900 mb-2">No orders yet</h3>
              <p class="text-gray-500">Orders will appear here once customers start placing them.</p>
            </div>
          <% else %>
            <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
              <div class="overflow-x-auto">
                <table class="min-w-full divide-y divide-gray-200">
                  <thead class="bg-gray-50">
                    <tr>
                      <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                        Order Number
                      </th>
                      <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                        Customer
                      </th>
                      <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                        Total
                      </th>
                      <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                        Payment
                      </th>
                      <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                        Fulfillment
                      </th>
                      <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                        Date
                      </th>
                    </tr>
                  </thead>
                  <tbody id="orders" phx-update="stream" class="divide-y divide-gray-100">
                    <tr
                      :for={{id, order} <- @streams.orders}
                      id={id}
                      class="hover:bg-gray-50 cursor-pointer"
                      phx-click={JS.navigate(~p"/admin/events/#{@event.id}/orders/#{order.id}")}
                    >
                      <td class="px-6 py-4 text-sm font-medium text-gray-900">
                        {order.order_number}
                      </td>
                      <td class="px-6 py-4 text-sm text-gray-600">
                        {order.customer_name}
                      </td>
                      <td class="px-6 py-4 text-sm text-gray-900 font-medium">
                        {format_price(order.total_cents)}
                      </td>
                      <td class="px-6 py-4 text-sm">
                        <.badge color={payment_badge_color(order.payment_status)} size="extra_small">
                          {Phoenix.Naming.humanize(order.payment_status)}
                        </.badge>
                      </td>
                      <td class="px-6 py-4 text-sm text-gray-600">
                        {fulfillment_summary(order)}
                      </td>
                      <td class="px-6 py-4 text-sm text-gray-500">
                        {Calendar.strftime(order.inserted_at, "%b %d, %Y %I:%M %p")}
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>

            </div>
          <% end %>
        </div>
      </div>
    </Layouts.admin>
    """
  end

  @impl true
  def mount(%{"event_id" => event_id}, _session, socket) do
    event = Ash.get!(PhotoFinish.Events.Event, event_id)
    orders = load_orders(event_id, "")

    {:ok,
     socket
     |> assign(:page_title, "Orders - #{event.name}")
     |> assign(:event, event)
     |> assign(:search_query, "")
     |> assign(:order_count, length(orders))
     |> stream(:orders, orders)}
  end

  @impl true
  def handle_event("search", %{"query" => query}, socket) do
    orders = load_orders(socket.assigns.event.id, query)

    {:noreply,
     socket
     |> assign(:search_query, query)
     |> stream(:orders, orders, reset: true)}
  end

  def handle_event("clear_search", _params, socket) do
    orders = load_orders(socket.assigns.event.id, "")

    {:noreply,
     socket
     |> assign(:search_query, "")
     |> stream(:orders, orders, reset: true)}
  end

  defp load_orders(event_id, query) do
    orders =
      Order
      |> Ash.Query.filter(event_id == ^event_id)
      |> Ash.Query.sort(inserted_at: :desc)
      |> Ash.read!()
      |> Ash.load!(:order_items)

    if query == "" do
      orders
    else
      downcased = String.downcase(query)

      Enum.filter(orders, fn order ->
        String.contains?(String.downcase(order.order_number), downcased) ||
          String.contains?(String.downcase(order.customer_name || ""), downcased)
      end)
    end
  end

  defp format_price(cents) when is_integer(cents) do
    "$#{:erlang.float_to_binary(cents / 100, decimals: 2)}"
  end

  defp format_price(_), do: "$0.00"

  defp payment_badge_color(:pending), do: "warning"
  defp payment_badge_color(:paid), do: "success"
  defp payment_badge_color(:refunded), do: "natural"
  defp payment_badge_color(_), do: "natural"

  defp fulfillment_summary(order) do
    items = order.order_items || []
    total = length(items)

    if total == 0 do
      "No items"
    else
      fulfilled = Enum.count(items, &(&1.fulfillment_status == :fulfilled))

      if fulfilled == total do
        "All fulfilled"
      else
        "#{fulfilled}/#{total} fulfilled"
      end
    end
  end
end
