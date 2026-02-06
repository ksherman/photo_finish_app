defmodule PhotoFinish.Orders.PlaceOrderTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.{Event, Competitor, EventCompetitor}
  alias PhotoFinish.Orders
  alias PhotoFinish.Orders.{ProductTemplate, EventProduct}

  setup do
    {:ok, event} =
      Ash.create(Event, %{
        name: "Spring Invitational 2026",
        slug: "spring-inv-2026",
        storage_root: "/photos/spring-2026",
        order_code: "STV",
        tax_rate_basis_points: 850
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

    {:ok, print_template} =
      Ash.create(ProductTemplate, %{
        product_type: :print,
        product_name: "8x10 Print",
        default_price_cents: 2500
      })

    {:ok, usb_template} =
      Ash.create(ProductTemplate, %{
        product_type: :usb,
        product_name: "Digital USB",
        default_price_cents: 4000
      })

    {:ok, print_product} =
      Ash.create(EventProduct, %{
        event_id: event.id,
        product_template_id: print_template.id,
        price_cents: 3000
      })

    {:ok, usb_product} =
      Ash.create(EventProduct, %{
        event_id: event.id,
        product_template_id: usb_template.id,
        price_cents: 5000
      })

    %{
      event: event,
      event_competitor: event_competitor,
      print_product: print_product,
      usb_product: usb_product
    }
  end

  describe "place_order/1" do
    test "creates an order with correct order number", %{
      event: event,
      event_competitor: ec,
      print_product: print
    } do
      {:ok, order} =
        Orders.place_order(%{
          event_id: event.id,
          customer_name: "Jane Doe",
          items: [
            %{event_product_id: print.id, event_competitor_id: ec.id}
          ]
        })

      assert order.order_number == "STV-0001"
    end

    test "calculates correct financial totals for single item", %{
      event: event,
      event_competitor: ec,
      print_product: print
    } do
      {:ok, order} =
        Orders.place_order(%{
          event_id: event.id,
          customer_name: "Jane Doe",
          items: [
            %{event_product_id: print.id, event_competitor_id: ec.id}
          ]
        })

      # print product is 3000 cents ($30.00)
      assert order.subtotal_cents == 3000
      # tax: 3000 * 850 / 10000 = 255 cents ($2.55)
      assert order.tax_rate_basis_points == 850
      assert order.tax_cents == 255
      # total: 3000 + 255 = 3255 cents ($32.55)
      assert order.total_cents == 3255
    end

    test "calculates correct totals for multiple items", %{
      event: event,
      event_competitor: ec,
      print_product: print,
      usb_product: usb
    } do
      {:ok, order} =
        Orders.place_order(%{
          event_id: event.id,
          customer_name: "John Smith",
          customer_email: "john@example.com",
          customer_phone: "555-9876",
          items: [
            %{event_product_id: print.id, event_competitor_id: ec.id},
            %{event_product_id: usb.id, event_competitor_id: ec.id}
          ]
        })

      # subtotal: 3000 + 5000 = 8000
      assert order.subtotal_cents == 8000
      # tax: 8000 * 850 / 10000 = 680
      assert order.tax_cents == 680
      # total: 8000 + 680 = 8680
      assert order.total_cents == 8680
      assert order.customer_email == "john@example.com"
      assert order.customer_phone == "555-9876"
    end

    test "creates order items with correct details", %{
      event: event,
      event_competitor: ec,
      print_product: print,
      usb_product: usb
    } do
      {:ok, order} =
        Orders.place_order(%{
          event_id: event.id,
          customer_name: "Jane Doe",
          items: [
            %{event_product_id: print.id, event_competitor_id: ec.id},
            %{event_product_id: usb.id, event_competitor_id: ec.id}
          ]
        })

      assert length(order.order_items) == 2

      print_item = Enum.find(order.order_items, &(&1.event_product_id == print.id))
      usb_item = Enum.find(order.order_items, &(&1.event_product_id == usb.id))

      assert print_item.unit_price_cents == 3000
      assert print_item.line_total_cents == 3000
      assert print_item.quantity == 1
      assert print_item.event_competitor_id == ec.id

      assert usb_item.unit_price_cents == 5000
      assert usb_item.line_total_cents == 5000
      assert usb_item.quantity == 1
    end

    test "sequential orders get incrementing order numbers", %{
      event: event,
      event_competitor: ec,
      print_product: print
    } do
      {:ok, order1} =
        Orders.place_order(%{
          event_id: event.id,
          customer_name: "Customer One",
          items: [%{event_product_id: print.id, event_competitor_id: ec.id}]
        })

      {:ok, order2} =
        Orders.place_order(%{
          event_id: event.id,
          customer_name: "Customer Two",
          items: [%{event_product_id: print.id, event_competitor_id: ec.id}]
        })

      assert order1.order_number == "STV-0001"
      assert order2.order_number == "STV-0002"
    end

    test "defaults payment_status to pending", %{
      event: event,
      event_competitor: ec,
      print_product: print
    } do
      {:ok, order} =
        Orders.place_order(%{
          event_id: event.id,
          customer_name: "Jane Doe",
          items: [%{event_product_id: print.id, event_competitor_id: ec.id}]
        })

      assert order.payment_status == :pending
    end

    test "returns error for non-existent event", %{
      event_competitor: ec,
      print_product: print
    } do
      result =
        Orders.place_order(%{
          event_id: "evt_nonexistent",
          customer_name: "Jane Doe",
          items: [%{event_product_id: print.id, event_competitor_id: ec.id}]
        })

      assert {:error, _} = result
    end

    test "returns error for non-existent event product", %{
      event: event,
      event_competitor: ec
    } do
      result =
        Orders.place_order(%{
          event_id: event.id,
          customer_name: "Jane Doe",
          items: [%{event_product_id: "evp_nonexistent", event_competitor_id: ec.id}]
        })

      assert {:error, _} = result
    end

    test "handles order with quantity > 1", %{
      event: event,
      event_competitor: ec,
      print_product: print
    } do
      {:ok, order} =
        Orders.place_order(%{
          event_id: event.id,
          customer_name: "Jane Doe",
          items: [
            %{event_product_id: print.id, event_competitor_id: ec.id, quantity: 3}
          ]
        })

      # 3 x 3000 = 9000
      assert order.subtotal_cents == 9000
      # tax: 9000 * 850 / 10000 = 765
      assert order.tax_cents == 765
      # total: 9000 + 765 = 9765
      assert order.total_cents == 9765

      item = hd(order.order_items)
      assert item.quantity == 3
      assert item.unit_price_cents == 3000
      assert item.line_total_cents == 9000
    end
  end
end
