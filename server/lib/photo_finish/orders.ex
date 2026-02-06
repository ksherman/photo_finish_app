defmodule PhotoFinish.Orders do
  use Ash.Domain,
    otp_app: :photo_finish

  resources do
    resource PhotoFinish.Orders.ProductTemplate
    resource PhotoFinish.Orders.EventProduct
    resource PhotoFinish.Orders.Order
    resource PhotoFinish.Orders.OrderItem
  end

  require Ash.Query

  alias PhotoFinish.Repo
  alias PhotoFinish.Events.Event
  alias PhotoFinish.Orders.{EventProduct, ProductTemplate, Order, OrderItem, OrderNumber}

  @doc """
  Initializes event products for an event by copying all active product templates.

  Creates an EventProduct for each active ProductTemplate, using the template's
  default_price_cents as the initial price_cents. Idempotent — if the event
  already has products, returns the existing ones without creating duplicates.

  ## Parameters

    * `event_id` - The event ID to initialize products for

  ## Returns

    * `{:ok, event_products}` - List of EventProducts (with product_template loaded)
    * `{:error, reason}` - On failure
  """
  @spec initialize_event_products(String.t()) :: {:ok, [EventProduct.t()]} | {:error, term()}
  def initialize_event_products(event_id) do
    existing =
      EventProduct
      |> Ash.Query.filter(event_id == ^event_id)
      |> Ash.read!()

    if existing != [] do
      {:ok, Ash.load!(existing, :product_template)}
    else
      active_templates =
        ProductTemplate
        |> Ash.Query.filter(is_active == true)
        |> Ash.read!()

      if active_templates == [] do
        {:ok, []}
      else
        event_products =
          Enum.map(active_templates, fn template ->
            Ash.create!(EventProduct, %{
              event_id: event_id,
              product_template_id: template.id,
              price_cents: template.default_price_cents
            })
          end)

        {:ok, Ash.load!(event_products, :product_template)}
      end
    end
  rescue
    e -> {:error, e}
  end

  @doc """
  Places a new order within a single database transaction.

  Generates a sequential order number, calculates financial totals
  (subtotal, tax, total), creates the Order record, and creates
  OrderItem records for each item.

  ## Parameters

  Accepts a map with:

    * `:event_id` (required) - The event ID
    * `:customer_name` (required) - Customer's name
    * `:customer_email` (optional) - Customer's email
    * `:customer_phone` (optional) - Customer's phone
    * `:items` (required) - List of maps with `:event_product_id` and `:event_competitor_id`

  ## Returns

    * `{:ok, order}` with the created order (items preloaded)
    * `{:error, reason}` on failure
  """
  @spec place_order(map()) :: {:ok, Order.t()} | {:error, term()}
  def place_order(params) do
    Repo.transaction(fn ->
      with {:ok, event} <- load_event(params.event_id),
           {:ok, event_products} <- load_event_products(params.items),
           {:ok, order_number} <- OrderNumber.generate(event.id),
           {subtotal_cents, item_details} <- calculate_items(params.items, event_products),
           tax_cents <- calculate_tax(subtotal_cents, event.tax_rate_basis_points),
           total_cents <- subtotal_cents + tax_cents,
           {:ok, order} <-
             create_order(%{
               event_id: event.id,
               order_number: order_number,
               customer_name: params.customer_name,
               customer_email: Map.get(params, :customer_email),
               customer_phone: Map.get(params, :customer_phone),
               subtotal_cents: subtotal_cents,
               tax_rate_basis_points: event.tax_rate_basis_points,
               tax_cents: tax_cents,
               total_cents: total_cents
             }),
           {:ok, _items} <- create_order_items(order.id, item_details) do
        Ash.load!(order, :order_items)
      else
        {:error, reason} -> Repo.rollback(reason)
      end
    end)
  end

  defp load_event(event_id) do
    case Ash.get(Event, event_id) do
      {:ok, event} -> {:ok, event}
      {:error, _} -> {:error, :event_not_found}
    end
  end

  defp load_event_products(items) do
    product_ids = Enum.map(items, & &1.event_product_id) |> Enum.uniq()

    products =
      EventProduct
      |> Ash.read!()
      |> Enum.filter(&(&1.id in product_ids))
      |> Map.new(&{&1.id, &1})

    missing = product_ids -- Map.keys(products)

    if missing == [] do
      {:ok, products}
    else
      {:error, {:event_products_not_found, missing}}
    end
  end

  defp calculate_items(items, event_products) do
    item_details =
      Enum.map(items, fn item ->
        product = Map.fetch!(event_products, item.event_product_id)
        quantity = Map.get(item, :quantity, 1)
        unit_price_cents = product.price_cents
        line_total_cents = quantity * unit_price_cents

        %{
          event_product_id: item.event_product_id,
          event_competitor_id: item.event_competitor_id,
          quantity: quantity,
          unit_price_cents: unit_price_cents,
          line_total_cents: line_total_cents
        }
      end)

    subtotal_cents = Enum.reduce(item_details, 0, &(&1.line_total_cents + &2))

    {subtotal_cents, item_details}
  end

  defp calculate_tax(subtotal_cents, tax_rate_basis_points) do
    round(subtotal_cents * tax_rate_basis_points / 10_000)
  end

  defp create_order(attrs) do
    Ash.create(Order, attrs)
  end

  defp create_order_items(order_id, item_details) do
    items =
      Enum.map(item_details, fn detail ->
        Ash.create!(OrderItem, Map.put(detail, :order_id, order_id))
      end)

    {:ok, items}
  rescue
    e -> {:error, e}
  end
end
