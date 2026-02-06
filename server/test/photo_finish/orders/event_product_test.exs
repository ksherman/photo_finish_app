defmodule PhotoFinish.Orders.EventProductTest do
  use PhotoFinish.DataCase, async: true

  alias PhotoFinish.Events.Event
  alias PhotoFinish.Orders.{ProductTemplate, EventProduct}

  setup do
    {:ok, event} =
      Ash.create(Event, %{
        name: "Test Event",
        slug: "test-event",
        storage_root: "/path/to/photos"
      })

    {:ok, template} =
      Ash.create(ProductTemplate, %{
        product_type: :print,
        product_name: "8x10 Print",
        default_price_cents: 2500
      })

    %{event: event, template: template}
  end

  describe "create" do
    test "generates ID with evp_ prefix", %{event: event, template: template} do
      {:ok, event_product} =
        Ash.create(EventProduct, %{
          event_id: event.id,
          product_template_id: template.id,
          price_cents: 3000
        })

      assert String.starts_with?(event_product.id, "evp_")
      suffix = String.replace_prefix(event_product.id, "evp_", "")
      assert Regex.match?(~r/^[a-z]{3}[0-9]{4}$/, suffix)
    end

    test "creates event product with required fields", %{event: event, template: template} do
      {:ok, event_product} =
        Ash.create(EventProduct, %{
          event_id: event.id,
          product_template_id: template.id,
          price_cents: 3000
        })

      assert event_product.event_id == event.id
      assert event_product.product_template_id == template.id
      assert event_product.price_cents == 3000
    end

    test "defaults is_available to true", %{event: event, template: template} do
      {:ok, event_product} =
        Ash.create(EventProduct, %{
          event_id: event.id,
          product_template_id: template.id,
          price_cents: 3000
        })

      assert event_product.is_available == true
    end

    test "requires price_cents", %{event: event, template: template} do
      result =
        Ash.create(EventProduct, %{
          event_id: event.id,
          product_template_id: template.id
        })

      assert {:error, _} = result
    end

    test "enforces unique event + product_template", %{event: event, template: template} do
      {:ok, _} =
        Ash.create(EventProduct, %{
          event_id: event.id,
          product_template_id: template.id,
          price_cents: 3000
        })

      result =
        Ash.create(EventProduct, %{
          event_id: event.id,
          product_template_id: template.id,
          price_cents: 3500
        })

      assert {:error, _} = result
    end

    test "allows same template in different events", %{template: template} do
      {:ok, event1} =
        Ash.create(Event, %{
          name: "Event 1",
          slug: "event-1",
          storage_root: "/photos/event1"
        })

      {:ok, event2} =
        Ash.create(Event, %{
          name: "Event 2",
          slug: "event-2",
          storage_root: "/photos/event2"
        })

      {:ok, ep1} =
        Ash.create(EventProduct, %{
          event_id: event1.id,
          product_template_id: template.id,
          price_cents: 3000
        })

      {:ok, ep2} =
        Ash.create(EventProduct, %{
          event_id: event2.id,
          product_template_id: template.id,
          price_cents: 3500
        })

      assert ep1.event_id != ep2.event_id
      assert ep1.product_template_id == ep2.product_template_id
    end
  end

  describe "update" do
    test "updates price and availability", %{event: event, template: template} do
      {:ok, event_product} =
        Ash.create(EventProduct, %{
          event_id: event.id,
          product_template_id: template.id,
          price_cents: 3000
        })

      {:ok, updated} =
        Ash.update(event_product, %{
          price_cents: 3500,
          is_available: false
        })

      assert updated.price_cents == 3500
      assert updated.is_available == false
    end
  end
end
