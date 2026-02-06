defmodule PhotoFinish.Orders.OrderItemTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.{Event, Competitor, EventCompetitor}
  alias PhotoFinish.Orders.{ProductTemplate, EventProduct, Order, OrderItem}

  setup do
    {:ok, event} =
      Ash.create(Event, %{
        name: "Test Event",
        slug: "test-event",
        storage_root: "/path/to/photos"
      })

    {:ok, competitor} =
      Ash.create(Competitor, %{
        first_name: "Jane",
        last_name: "Doe"
      })

    {:ok, event_competitor} =
      Ash.create(EventCompetitor, %{
        event_id: event.id,
        competitor_id: competitor.id,
        competitor_number: "101"
      })

    {:ok, template} =
      Ash.create(ProductTemplate, %{
        product_type: :print,
        product_name: "8x10 Print",
        default_price_cents: 2500
      })

    {:ok, event_product} =
      Ash.create(EventProduct, %{
        event_id: event.id,
        product_template_id: template.id,
        price_cents: 3000
      })

    {:ok, order} =
      Ash.create(Order, %{
        event_id: event.id,
        order_number: "ORD-001",
        customer_name: "Jane Doe",
        subtotal_cents: 3000,
        tax_rate_basis_points: 850,
        tax_cents: 255,
        total_cents: 3255
      })

    %{
      event: event,
      event_competitor: event_competitor,
      event_product: event_product,
      order: order
    }
  end

  describe "create" do
    test "generates ID with itm_ prefix", %{
      order: order,
      event_product: event_product,
      event_competitor: event_competitor
    } do
      {:ok, item} =
        Ash.create(OrderItem, %{
          order_id: order.id,
          event_product_id: event_product.id,
          event_competitor_id: event_competitor.id,
          unit_price_cents: 3000,
          line_total_cents: 3000
        })

      assert String.starts_with?(item.id, "itm_")
      suffix = String.replace_prefix(item.id, "itm_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end

    test "creates order item with all fields", %{
      order: order,
      event_product: event_product,
      event_competitor: event_competitor
    } do
      {:ok, item} =
        Ash.create(OrderItem, %{
          order_id: order.id,
          event_product_id: event_product.id,
          event_competitor_id: event_competitor.id,
          quantity: 2,
          unit_price_cents: 3000,
          line_total_cents: 6000
        })

      assert item.order_id == order.id
      assert item.event_product_id == event_product.id
      assert item.event_competitor_id == event_competitor.id
      assert item.quantity == 2
      assert item.unit_price_cents == 3000
      assert item.line_total_cents == 6000
    end

    test "defaults quantity to 1", %{
      order: order,
      event_product: event_product,
      event_competitor: event_competitor
    } do
      {:ok, item} =
        Ash.create(OrderItem, %{
          order_id: order.id,
          event_product_id: event_product.id,
          event_competitor_id: event_competitor.id,
          unit_price_cents: 3000,
          line_total_cents: 3000
        })

      assert item.quantity == 1
    end

    test "defaults fulfillment_status to pending", %{
      order: order,
      event_product: event_product,
      event_competitor: event_competitor
    } do
      {:ok, item} =
        Ash.create(OrderItem, %{
          order_id: order.id,
          event_product_id: event_product.id,
          event_competitor_id: event_competitor.id,
          unit_price_cents: 3000,
          line_total_cents: 3000
        })

      assert item.fulfillment_status == :pending
    end

    test "requires unit_price_cents", %{
      order: order,
      event_product: event_product,
      event_competitor: event_competitor
    } do
      result =
        Ash.create(OrderItem, %{
          order_id: order.id,
          event_product_id: event_product.id,
          event_competitor_id: event_competitor.id,
          line_total_cents: 3000
        })

      assert {:error, _} = result
    end

    test "requires line_total_cents", %{
      order: order,
      event_product: event_product,
      event_competitor: event_competitor
    } do
      result =
        Ash.create(OrderItem, %{
          order_id: order.id,
          event_product_id: event_product.id,
          event_competitor_id: event_competitor.id,
          unit_price_cents: 3000
        })

      assert {:error, _} = result
    end
  end

  describe "update" do
    test "updates fulfillment_status", %{
      order: order,
      event_product: event_product,
      event_competitor: event_competitor
    } do
      {:ok, item} =
        Ash.create(OrderItem, %{
          order_id: order.id,
          event_product_id: event_product.id,
          event_competitor_id: event_competitor.id,
          unit_price_cents: 3000,
          line_total_cents: 3000
        })

      {:ok, updated} = Ash.update(item, %{fulfillment_status: :fulfilled})
      assert updated.fulfillment_status == :fulfilled
    end
  end
end
