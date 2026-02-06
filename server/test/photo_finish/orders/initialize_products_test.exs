defmodule PhotoFinish.Orders.InitializeProductsTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.Event
  alias PhotoFinish.Orders
  alias PhotoFinish.Orders.{ProductTemplate, EventProduct}

  setup do
    {:ok, event} =
      Ash.create(Event, %{
        name: "Test Meet 2026",
        slug: "test-meet-2026",
        storage_root: "/photos/test-2026"
      })

    {:ok, active_print} =
      Ash.create(ProductTemplate, %{
        product_type: :print,
        product_name: "8x10 Print",
        default_price_cents: 2500,
        is_active: true
      })

    {:ok, active_usb} =
      Ash.create(ProductTemplate, %{
        product_type: :usb,
        product_name: "Digital USB",
        default_price_cents: 4000,
        is_active: true
      })

    {:ok, _inactive} =
      Ash.create(ProductTemplate, %{
        product_type: :collage,
        product_name: "Photo Collage",
        default_price_cents: 5000,
        is_active: false
      })

    %{
      event: event,
      active_print: active_print,
      active_usb: active_usb
    }
  end

  describe "initialize_event_products/1" do
    test "creates event products for each active template with correct prices", %{
      event: event,
      active_print: print_template,
      active_usb: usb_template
    } do
      {:ok, event_products} = Orders.initialize_event_products(event.id)

      assert length(event_products) == 2

      print_ep = Enum.find(event_products, &(&1.product_template_id == print_template.id))
      usb_ep = Enum.find(event_products, &(&1.product_template_id == usb_template.id))

      assert print_ep != nil
      assert print_ep.price_cents == 2500
      assert print_ep.is_available == true
      assert print_ep.event_id == event.id

      assert usb_ep != nil
      assert usb_ep.price_cents == 4000
      assert usb_ep.is_available == true
      assert usb_ep.event_id == event.id
    end

    test "only includes active templates (excludes inactive)", %{event: event} do
      {:ok, event_products} = Orders.initialize_event_products(event.id)

      template_types =
        event_products
        |> Enum.map(& &1.product_template.product_type)
        |> Enum.sort()

      assert template_types == [:print, :usb]
      refute Enum.any?(event_products, fn ep -> ep.product_template.product_type == :collage end)
    end

    test "loads product_template relationship on returned products", %{event: event} do
      {:ok, event_products} = Orders.initialize_event_products(event.id)

      Enum.each(event_products, fn ep ->
        assert ep.product_template != nil
        assert ep.product_template.product_name != nil
      end)
    end

    test "is idempotent — calling twice does not duplicate products", %{event: event} do
      {:ok, first_call} = Orders.initialize_event_products(event.id)
      {:ok, second_call} = Orders.initialize_event_products(event.id)

      assert length(first_call) == length(second_call)

      first_ids = Enum.map(first_call, & &1.id) |> Enum.sort()
      second_ids = Enum.map(second_call, & &1.id) |> Enum.sort()
      assert first_ids == second_ids
    end

    test "returns existing products when event already has products", %{event: event} do
      # Manually create one product first
      {:ok, template} =
        Ash.create(ProductTemplate, %{
          product_type: :accessory,
          product_name: "Keychain",
          default_price_cents: 1000,
          is_active: true
        })

      {:ok, existing_ep} =
        Ash.create(EventProduct, %{
          event_id: event.id,
          product_template_id: template.id,
          price_cents: 1500
        })

      {:ok, result} = Orders.initialize_event_products(event.id)

      # Should return just the one existing product, not create more
      assert length(result) == 1
      assert hd(result).id == existing_ep.id
      assert hd(result).price_cents == 1500
    end

    test "returns empty list when no active templates exist" do
      {:ok, event} =
        Ash.create(Event, %{
          name: "Empty Event",
          slug: "empty-event",
          storage_root: "/photos/empty"
        })

      # Deactivate all templates first — create a fresh event with no active templates
      # We use a separate event to avoid the templates from the setup block
      # The setup block creates templates, but this event has no products
      # Since the event has no existing products and we need to test with no active templates,
      # we deactivate the ones from setup
      ProductTemplate
      |> Ash.read!()
      |> Enum.each(fn t -> Ash.update!(t, %{is_active: false}) end)

      {:ok, result} = Orders.initialize_event_products(event.id)
      assert result == []
    end
  end
end
