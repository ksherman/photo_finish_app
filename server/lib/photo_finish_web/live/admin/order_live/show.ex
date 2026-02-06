defmodule PhotoFinishWeb.Admin.OrderLive.Show do
  use PhotoFinishWeb, :live_view

  alias PhotoFinish.Orders.Order

  @impl true
  def render(assigns) do
    ~H"""
    <Layouts.admin flash={@flash}>
      <div class="min-h-screen bg-gray-50">
        <%!-- Header --%>
        <div class="bg-white border-b border-gray-200 px-6 py-4">
          <div class="max-w-4xl mx-auto">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-4">
                <.button_link
                  navigate={~p"/admin/events/#{@event.id}/orders"}
                  variant="outline"
                  color="natural"
                  size="small"
                >
                  <.icon name="hero-arrow-left" class="w-4 h-4" />
                </.button_link>
                <div>
                  <h1 class="text-xl font-bold text-gray-900">
                    Order {@order.order_number}
                  </h1>
                  <p class="text-sm text-gray-500">
                    Placed {Calendar.strftime(@order.inserted_at, "%B %d, %Y at %I:%M %p")}
                  </p>
                </div>
                <.badge color={payment_badge_color(@order.payment_status)}>
                  {Phoenix.Naming.humanize(@order.payment_status)}
                </.badge>
              </div>
            </div>
          </div>
        </div>

        <%!-- Main Content --%>
        <div class="max-w-4xl mx-auto py-8 px-6 space-y-6">
          <%!-- Customer Info Card --%>
          <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
            <div class="px-6 py-4 bg-gray-50 border-b border-gray-200">
              <h2 class="text-lg font-semibold text-gray-900">Customer Information</h2>
            </div>
            <div class="p-6">
              <dl class="grid grid-cols-1 sm:grid-cols-3 gap-x-8 gap-y-4">
                <div>
                  <dt class="text-sm font-medium text-gray-500">Name</dt>
                  <dd class="mt-1 text-sm text-gray-900">{@order.customer_name}</dd>
                </div>
                <div>
                  <dt class="text-sm font-medium text-gray-500">Email</dt>
                  <dd class="mt-1 text-sm text-gray-900">{@order.customer_email || "-"}</dd>
                </div>
                <div>
                  <dt class="text-sm font-medium text-gray-500">Phone</dt>
                  <dd class="mt-1 text-sm text-gray-900">{@order.customer_phone || "-"}</dd>
                </div>
              </dl>
            </div>
          </div>

          <%!-- Financial Summary Card --%>
          <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
            <div class="px-6 py-4 bg-gray-50 border-b border-gray-200">
              <h2 class="text-lg font-semibold text-gray-900">Financial Summary</h2>
            </div>
            <div class="p-6">
              <dl class="space-y-3">
                <div class="flex justify-between text-sm">
                  <dt class="text-gray-500">Subtotal</dt>
                  <dd class="text-gray-900">{format_price(@order.subtotal_cents)}</dd>
                </div>
                <div class="flex justify-between text-sm">
                  <dt class="text-gray-500">
                    Tax ({format_tax_rate(@order.tax_rate_basis_points)})
                  </dt>
                  <dd class="text-gray-900">{format_price(@order.tax_cents)}</dd>
                </div>
                <div class="flex justify-between text-sm font-semibold border-t border-gray-200 pt-3">
                  <dt class="text-gray-900">Total</dt>
                  <dd class="text-gray-900">{format_price(@order.total_cents)}</dd>
                </div>
              </dl>
            </div>
          </div>

          <%!-- Payment Section --%>
          <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
            <div class="px-6 py-4 bg-gray-50 border-b border-gray-200">
              <div class="flex items-center justify-between">
                <h2 class="text-lg font-semibold text-gray-900">Payment</h2>
                <.badge color={payment_badge_color(@order.payment_status)}>
                  {Phoenix.Naming.humanize(@order.payment_status)}
                </.badge>
              </div>
            </div>
            <div class="p-6">
              <%= if @order.payment_status == :pending do %>
                <form phx-submit="mark_paid" class="space-y-4">
                  <div>
                    <label for="payment_reference" class="block text-sm font-medium text-gray-700">
                      Payment Reference
                    </label>
                    <input
                      type="text"
                      id="payment_reference"
                      name="payment_reference"
                      placeholder="e.g. Cash, Check #1234, Card ending 5678"
                      class="mt-1 block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 text-sm"
                    />
                  </div>
                  <.button type="submit" color="success" size="small">
                    <.icon name="hero-check" class="w-4 h-4 mr-1" /> Mark as Paid
                  </.button>
                </form>
              <% else %>
                <dl class="space-y-3">
                  <%= if @order.payment_reference do %>
                    <div>
                      <dt class="text-sm font-medium text-gray-500">Payment Reference</dt>
                      <dd class="mt-1 text-sm text-gray-900">{@order.payment_reference}</dd>
                    </div>
                  <% end %>
                  <div>
                    <dt class="text-sm font-medium text-gray-500">Updated</dt>
                    <dd class="mt-1 text-sm text-gray-900">
                      {Calendar.strftime(@order.updated_at, "%B %d, %Y at %I:%M %p")}
                    </dd>
                  </div>
                </dl>
              <% end %>
            </div>
          </div>

          <%!-- Notes Section --%>
          <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
            <div class="px-6 py-4 bg-gray-50 border-b border-gray-200">
              <h2 class="text-lg font-semibold text-gray-900">Notes</h2>
            </div>
            <div class="p-6">
              <form phx-submit="update_notes">
                <textarea
                  name="notes"
                  rows="3"
                  placeholder="Add internal notes about this order..."
                  class="block w-full rounded-md border-gray-300 shadow-sm focus:border-blue-500 focus:ring-blue-500 text-sm"
                >{@order.notes}</textarea>
                <div class="mt-3">
                  <.button type="submit" size="small" variant="outline" color="natural">
                    <.icon name="hero-pencil" class="w-4 h-4 mr-1" /> Save Notes
                  </.button>
                </div>
              </form>
            </div>
          </div>

          <%!-- Order Items Table --%>
          <div class="bg-white rounded-lg border border-gray-200 overflow-hidden">
            <div class="px-6 py-4 bg-gray-50 border-b border-gray-200">
              <h2 class="text-lg font-semibold text-gray-900">
                Items ({length(@order.order_items)})
              </h2>
            </div>
            <div class="overflow-x-auto">
              <table class="min-w-full divide-y divide-gray-200">
                <thead class="bg-gray-50">
                  <tr>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Product
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Competitor
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Qty
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Price
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Status
                    </th>
                    <th class="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                      Actions
                    </th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-gray-100">
                  <%= for item <- @order.order_items do %>
                    <tr class="hover:bg-gray-50">
                      <td class="px-6 py-4 text-sm text-gray-900 font-medium">
                        {product_name(item)}
                      </td>
                      <td class="px-6 py-4 text-sm text-gray-600">
                        {competitor_display(item)}
                      </td>
                      <td class="px-6 py-4 text-sm text-gray-600">
                        {item.quantity}
                      </td>
                      <td class="px-6 py-4 text-sm text-gray-900">
                        {format_price(item.line_total_cents)}
                      </td>
                      <td class="px-6 py-4 text-sm">
                        <.badge
                          color={fulfillment_badge_color(item.fulfillment_status)}
                          size="extra_small"
                        >
                          {Phoenix.Naming.humanize(item.fulfillment_status)}
                        </.badge>
                      </td>
                      <td class="px-6 py-4 text-sm">
                        <%= if item.fulfillment_status == :pending do %>
                          <.button
                            phx-click="mark_fulfilled"
                            phx-value-item-id={item.id}
                            size="small"
                            variant="outline"
                            color="success"
                          >
                            <.icon name="hero-check" class="w-3 h-3 mr-1" /> Fulfill
                          </.button>
                        <% else %>
                          <span class="text-gray-400 text-xs">Done</span>
                        <% end %>
                      </td>
                    </tr>
                  <% end %>
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>
    </Layouts.admin>
    """
  end

  @impl true
  def mount(%{"event_id" => event_id, "id" => id}, _session, socket) do
    event = Ash.get!(PhotoFinish.Events.Event, event_id)
    order = load_order(id)

    {:ok,
     socket
     |> assign(:page_title, "Order #{order.order_number}")
     |> assign(:event, event)
     |> assign(:order, order)}
  end

  @impl true
  def handle_event("mark_paid", %{"payment_reference" => payment_reference}, socket) do
    order = socket.assigns.order

    case Ash.update(order, %{payment_status: :paid, payment_reference: payment_reference}) do
      {:ok, updated_order} ->
        order = load_order(updated_order.id)

        {:noreply,
         socket
         |> assign(:order, order)
         |> put_flash(:info, "Order marked as paid.")}

      {:error, reason} ->
        {:noreply, put_flash(socket, :error, "Failed to update payment: #{inspect(reason)}")}
    end
  end

  def handle_event("update_notes", %{"notes" => notes}, socket) do
    order = socket.assigns.order

    case Ash.update(order, %{notes: notes}) do
      {:ok, updated_order} ->
        order = load_order(updated_order.id)

        {:noreply,
         socket
         |> assign(:order, order)
         |> put_flash(:info, "Notes updated.")}

      {:error, reason} ->
        {:noreply, put_flash(socket, :error, "Failed to update notes: #{inspect(reason)}")}
    end
  end

  def handle_event("mark_fulfilled", %{"item-id" => item_id}, socket) do
    item = Enum.find(socket.assigns.order.order_items, &(&1.id == item_id))

    case Ash.update(item, %{fulfillment_status: :fulfilled}) do
      {:ok, _updated_item} ->
        order = load_order(socket.assigns.order.id)

        {:noreply,
         socket
         |> assign(:order, order)
         |> put_flash(:info, "Item marked as fulfilled.")}

      {:error, reason} ->
        {:noreply, put_flash(socket, :error, "Failed to update item: #{inspect(reason)}")}
    end
  end

  defp load_order(id) do
    Ash.get!(Order, id)
    |> Ash.load!([order_items: [event_product: [:product_template], event_competitor: []]])
  end

  defp format_price(cents) when is_integer(cents) do
    "$#{:erlang.float_to_binary(cents / 100, decimals: 2)}"
  end

  defp format_price(_), do: "$0.00"

  defp format_tax_rate(basis_points) when is_integer(basis_points) do
    "#{Float.round(basis_points / 100, 2)}%"
  end

  defp format_tax_rate(_), do: "0%"

  defp payment_badge_color(:pending), do: "warning"
  defp payment_badge_color(:paid), do: "success"
  defp payment_badge_color(:refunded), do: "natural"
  defp payment_badge_color(_), do: "natural"

  defp fulfillment_badge_color(:pending), do: "warning"
  defp fulfillment_badge_color(:fulfilled), do: "success"
  defp fulfillment_badge_color(_), do: "natural"

  defp product_name(item) do
    case item.event_product do
      %{product_template: %{product_name: name}} -> name
      _ -> "Unknown Product"
    end
  end

  defp competitor_display(item) do
    case item.event_competitor do
      %{display_name: name, competitor_number: number} when not is_nil(name) ->
        "#{name} (##{number})"

      %{competitor_number: number} ->
        "##{number}"

      _ ->
        "-"
    end
  end
end
