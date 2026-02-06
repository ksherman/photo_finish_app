defmodule PhotoFinishWeb.ViewerLive.Order do
  @moduledoc """
  Multi-step order flow for kiosk viewers.

  Steps:
    1. :review   — Show product summary and price
    2. :checkout — Collect customer info and place the order
    3. :confirmation — Display the order number for pickup
  """
  use PhotoFinishWeb, :live_view

  require Ash.Query

  alias PhotoFinish.Events.EventCompetitor
  alias PhotoFinish.Orders.EventProduct
  alias PhotoFinish.Photos.Photo

  @impl true
  def mount(%{"event_id" => event_id, "id" => id}, _session, socket) do
    event_competitor = Ash.get!(EventCompetitor, id)
    event = Ash.get!(PhotoFinish.Events.Event, event_competitor.event_id)

    usb_product = find_usb_product(event.id)

    photo_count =
      Photo
      |> Ash.Query.filter(event_competitor_id == ^id and status == :ready)
      |> Ash.read!()
      |> length()

    socket =
      socket
      |> assign(:event_id, event_id)
      |> assign(:event_competitor, event_competitor)
      |> assign(:event, event)
      |> assign(:usb_product, usb_product)
      |> assign(:photo_count, photo_count)
      |> assign(:step, :review)
      |> assign(:order, nil)
      |> assign(:form_data, %{"customer_name" => "", "customer_email" => "", "customer_phone" => ""})
      |> assign(:form_errors, %{})

    {:ok, socket}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <div class="min-h-screen bg-gray-100">
      <!-- Header -->
      <header class="bg-white shadow-sm">
        <div class="max-w-2xl mx-auto px-4 py-4 flex items-center gap-4">
          <.link
            :if={@step != :confirmation}
            navigate={~p"/viewer/#{@event_id}/competitor/#{@event_competitor.id}"}
            class="text-gray-500 hover:text-gray-700"
          >
            <.icon name="hero-arrow-left" class="w-6 h-6" />
          </.link>
          <h1 class="text-lg font-bold text-gray-900">Order Photos</h1>
        </div>
      </header>

      <main class="max-w-2xl mx-auto px-4 py-8">
        <%= case @step do %>
          <% :review -> %>
            {render_review(assigns)}
          <% :checkout -> %>
            {render_checkout(assigns)}
          <% :confirmation -> %>
            {render_confirmation(assigns)}
        <% end %>
      </main>
    </div>
    """
  end

  defp render_review(assigns) do
    ~H"""
    <div class="bg-white rounded-xl shadow-sm p-6 space-y-6">
      <div class="text-center">
        <h2 class="text-2xl font-bold text-gray-900">
          {@event_competitor.display_name || "Competitor #{@event_competitor.competitor_number}"}
        </h2>
        <p class="text-gray-500 mt-1">{@photo_count} photos</p>
      </div>

      <div class="border rounded-lg p-4 flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="bg-indigo-100 rounded-lg p-3">
            <.icon name="hero-circle-stack" class="w-6 h-6 text-indigo-600" />
          </div>
          <div>
            <p class="font-semibold text-gray-900">
              {if @usb_product, do: @usb_product.product_template.product_name, else: "All Photos USB Drive"}
            </p>
            <p class="text-sm text-gray-500">All {@photo_count} photos included</p>
          </div>
        </div>
        <p class="text-xl font-bold text-gray-900">
          {format_price(if @usb_product, do: @usb_product.price_cents, else: 0)}
        </p>
      </div>

      <button
        phx-click="continue_to_checkout"
        class="w-full py-4 bg-indigo-600 text-white text-lg font-semibold rounded-xl hover:bg-indigo-700 transition"
      >
        Continue to Checkout
      </button>
    </div>
    """
  end

  defp render_checkout(assigns) do
    subtotal = if assigns.usb_product, do: assigns.usb_product.price_cents, else: 0
    tax_cents = calculate_tax(subtotal, assigns.event.tax_rate_basis_points)
    total_cents = subtotal + tax_cents

    assigns =
      assigns
      |> Map.put(:subtotal, subtotal)
      |> Map.put(:tax_cents, tax_cents)
      |> Map.put(:total_cents, total_cents)

    ~H"""
    <div class="bg-white rounded-xl shadow-sm p-6 space-y-6">
      <h2 class="text-xl font-bold text-gray-900">Checkout</h2>

      <!-- Order Summary -->
      <div class="border rounded-lg p-4 space-y-2 bg-gray-50">
        <div class="flex justify-between text-sm text-gray-600">
          <span>
            {if @usb_product, do: @usb_product.product_template.product_name, else: "USB Drive"}
            for {@event_competitor.display_name || "Competitor #{@event_competitor.competitor_number}"}
          </span>
          <span>{format_price(@subtotal)}</span>
        </div>
        <div class="flex justify-between text-sm text-gray-600">
          <span>Tax ({format_tax_rate(@event.tax_rate_basis_points)})</span>
          <span>{format_price(@tax_cents)}</span>
        </div>
        <div class="border-t pt-2 flex justify-between font-bold text-gray-900">
          <span>Total</span>
          <span>{format_price(@total_cents)}</span>
        </div>
      </div>

      <!-- Customer Form -->
      <form phx-submit="place_order" class="space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">
            Your Name <span class="text-red-500">*</span>
          </label>
          <input
            type="text"
            name="customer_name"
            value={@form_data["customer_name"]}
            required
            placeholder="Enter your name"
            class={[
              "w-full px-4 py-3 text-lg rounded-lg border-2 focus:ring-0",
              if(@form_errors["customer_name"],
                do: "border-red-400 focus:border-red-500",
                else: "border-gray-200 focus:border-indigo-500"
              )
            ]}
          />
          <p :if={@form_errors["customer_name"]} class="mt-1 text-sm text-red-600">
            {@form_errors["customer_name"]}
          </p>
        </div>

        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">
            Email <span class="text-gray-400">(optional)</span>
          </label>
          <input
            type="email"
            name="customer_email"
            value={@form_data["customer_email"]}
            placeholder="you@example.com"
            class="w-full px-4 py-3 text-lg rounded-lg border-2 border-gray-200 focus:border-indigo-500 focus:ring-0"
          />
        </div>

        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">
            Phone <span class="text-gray-400">(optional)</span>
          </label>
          <input
            type="tel"
            name="customer_phone"
            value={@form_data["customer_phone"]}
            placeholder="(555) 123-4567"
            class="w-full px-4 py-3 text-lg rounded-lg border-2 border-gray-200 focus:border-indigo-500 focus:ring-0"
          />
        </div>

        <div class="flex gap-3 pt-2">
          <button
            type="button"
            phx-click="back_to_review"
            class="flex-1 py-4 border-2 border-gray-300 text-gray-700 text-lg font-semibold rounded-xl hover:bg-gray-50 transition"
          >
            Back
          </button>
          <button
            type="submit"
            class="flex-1 py-4 bg-indigo-600 text-white text-lg font-semibold rounded-xl hover:bg-indigo-700 transition"
          >
            Place Order
          </button>
        </div>
      </form>
    </div>
    """
  end

  defp render_confirmation(assigns) do
    ~H"""
    <div class="bg-white rounded-xl shadow-sm p-8 text-center space-y-6">
      <div class="bg-green-100 rounded-full w-16 h-16 flex items-center justify-center mx-auto">
        <.icon name="hero-check" class="w-8 h-8 text-green-600" />
      </div>

      <div>
        <p class="text-gray-500 text-lg">Your order number is</p>
        <p class="text-5xl font-extrabold text-indigo-600 mt-2 tracking-wide">
          {@order.order_number}
        </p>
      </div>

      <div class="border rounded-lg p-4 inline-block mx-auto">
        <p class="text-sm text-gray-500">Total</p>
        <p class="text-2xl font-bold text-gray-900">{format_price(@order.total_cents)}</p>
      </div>

      <div class="bg-amber-50 border border-amber-200 rounded-lg p-4">
        <p class="text-amber-800 font-semibold text-lg">
          <.icon name="hero-information-circle" class="w-5 h-5 inline mr-1" />
          Please take this number to the payment desk.
        </p>
      </div>

      <.link
        navigate={~p"/viewer/#{@event_id}"}
        class="inline-block w-full py-4 border-2 border-gray-300 text-gray-700 text-lg font-semibold rounded-xl hover:bg-gray-50 transition"
      >
        Start New Order
      </.link>
    </div>
    """
  end

  # -- Events ---------------------------------------------------------------

  @impl true
  def handle_event("continue_to_checkout", _params, socket) do
    {:noreply, assign(socket, :step, :checkout)}
  end

  def handle_event("back_to_review", _params, socket) do
    {:noreply, assign(socket, :step, :review)}
  end

  def handle_event("place_order", params, socket) do
    customer_name = String.trim(params["customer_name"] || "")
    customer_email = blank_to_nil(params["customer_email"])
    customer_phone = blank_to_nil(params["customer_phone"])

    form_data = %{
      "customer_name" => customer_name,
      "customer_email" => customer_email || "",
      "customer_phone" => customer_phone || ""
    }

    if customer_name == "" do
      {:noreply,
       socket
       |> assign(:form_data, form_data)
       |> assign(:form_errors, %{"customer_name" => "Name is required"})}
    else
      usb_product = socket.assigns.usb_product

      order_params = %{
        event_id: socket.assigns.event.id,
        customer_name: customer_name,
        customer_email: customer_email,
        customer_phone: customer_phone,
        items: [
          %{
            event_product_id: usb_product.id,
            event_competitor_id: socket.assigns.event_competitor.id
          }
        ]
      }

      case PhotoFinish.Orders.place_order(order_params) do
        {:ok, order} ->
          {:noreply,
           socket
           |> assign(:order, order)
           |> assign(:step, :confirmation)
           |> assign(:form_errors, %{})}

        {:error, reason} ->
          {:noreply,
           socket
           |> assign(:form_data, form_data)
           |> put_flash(:error, "Failed to place order: #{inspect(reason)}")}
      end
    end
  end

  # -- Helpers --------------------------------------------------------------

  defp find_usb_product(event_id) do
    event_products =
      EventProduct
      |> Ash.Query.filter(event_id == ^event_id)
      |> Ash.read!()
      |> Ash.load!(:product_template)

    Enum.find(event_products, fn ep ->
      ep.product_template && ep.product_template.product_type == :usb
    end)
  end

  defp format_price(cents) when is_integer(cents) do
    "$#{:erlang.float_to_binary(cents / 100, decimals: 2)}"
  end

  defp format_price(_), do: "$0.00"

  defp format_tax_rate(basis_points) do
    rate = basis_points / 100
    "#{:erlang.float_to_binary(rate, decimals: 2)}%"
  end

  defp calculate_tax(subtotal_cents, tax_rate_basis_points) do
    round(subtotal_cents * tax_rate_basis_points / 10_000)
  end

  defp blank_to_nil(nil), do: nil
  defp blank_to_nil(str) when is_binary(str) do
    case String.trim(str) do
      "" -> nil
      trimmed -> trimmed
    end
  end
end
